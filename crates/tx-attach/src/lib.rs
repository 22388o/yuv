#![doc = include_str!("../README.md")]

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

use bitcoin::{OutPoint, Txid};
use event_bus::{typeid, EventBus};
use eyre::WrapErr;
use tokio_util::sync::CancellationToken;

use yuv_storage::{FrozenTxsStorage, PagesStorage, TransactionsStorage};
use yuv_types::{
    ControllerMessage, FreezeTxToggle, GraphBuilderMessage, ProofMap, YuvTransaction, YuvTxType,
};

/// Service which handles attaching of transactions to the graph.
///
/// Accepts batches of checked transactions, and attaches
/// history of transactions, and if all dependencies (parents) are attached,
/// then marks transaction as attached, and stores it in [`TransactionsStorage`].
pub struct GraphBuilder<TransactionStorage, StateStorage> {
    /// Storage of transactions, where attached transactions are stored.
    tx_storage: TransactionStorage,

    /// A storage for state of the node.
    state_storage: StateStorage,

    /// Event bus for simplifying communication with services.
    event_bus: EventBus,

    /// Map of inverse dependencies between transactions. Key is a transaction
    /// id, and value is transactions that depend on this transaction.
    inverse_deps: HashMap<Txid, HashSet<Txid>>,

    /// Map of dependencies between transactions. Key is a transaction id, and
    /// value is transactions that this transaction depends on.
    deps: HashMap<Txid, HashSet<Txid>>,

    /// Stored txs that are not verified yet, with point in time in which
    /// transaction was stored.
    stored_txs: HashMap<Txid, (YuvTransaction, SystemTime)>,

    /// Period of time after which [`Self`] will cleanup transactions
    /// that are _too old_.
    cleanup_period: Duration,

    /// Period of time, after which we consider transaction _too old_
    /// or _outdated_.
    tx_outdated_duration: Duration,

    /// Amount of transactions that fit one page.
    tx_per_page: u64,
}

const DURATION_ONE_HOUR: Duration = Duration::from_secs(60 * 60);
const DURATION_ONE_DAY: Duration = Duration::from_secs(60 * 60 * 24);

impl<TS, SS> GraphBuilder<TS, SS>
where
    TS: TransactionsStorage + PagesStorage + Send + Sync + 'static,
    SS: FrozenTxsStorage + Send + Sync + 'static,
{
    pub fn new(
        tx_storage: TS,
        state_storage: SS,
        full_event_bus: &EventBus,
        tx_per_page: u64,
    ) -> Self {
        let event_bus = full_event_bus
            .extract(&typeid![ControllerMessage], &typeid![GraphBuilderMessage])
            .expect("event channels must be presented");

        Self {
            tx_storage,
            event_bus,
            state_storage,
            inverse_deps: Default::default(),
            deps: Default::default(),
            stored_txs: Default::default(),
            tx_per_page,
            cleanup_period: DURATION_ONE_HOUR,
            tx_outdated_duration: DURATION_ONE_DAY,
        }
    }

    /// Set period of time after each [`Self`] will delete all transactions
    /// _outdated_ transactions, see ([`self`](Self)) for more info.
    pub fn with_cleanup_period(mut self, period: Duration) -> Self {
        self.cleanup_period = period;
        self
    }

    /// Set time duration after which transaction is considered _outdated_
    /// for more info see [`self`](Self).
    pub fn with_outdated_duration(mut self, duration: Duration) -> Self {
        self.tx_outdated_duration = duration;
        self
    }

    /// Starts attach incoming [`transactions`](YuvTransaction).
    pub async fn run(mut self, cancellation: CancellationToken) {
        let events = self.event_bus.subscribe::<GraphBuilderMessage>();
        let mut timer = tokio::time::interval(self.cleanup_period);

        loop {
            tokio::select! {
                event = events.recv() => {
                    let Ok(event) = event else {
                        tracing::trace!("Channel for incoming events is dropped, stopping...");
                        return;
                    };

                    if let Err(err) = self.handle_event(event).await {
                        tracing::error!("Failed to handle event: {:?}", err);
                    }
                },
                _ = cancellation.cancelled() => {
                    tracing::trace!("Cancellation received, stopping graph builder");
                    return;
                },
                _ = timer.tick() => {
                    if let Err(err) = self.handle_cleanup().await {
                        tracing::error!("Failed to do cleanup: {:?}", err);
                    }
                }
            }
        }
    }

    /// Handles incoming [`events`](GraphBuilderMessage).
    async fn handle_event(&mut self, event: GraphBuilderMessage) -> eyre::Result<()> {
        match event {
            GraphBuilderMessage::CheckedTxs(txs) => self
                .attach_txs(&txs)
                .await
                .wrap_err("failed to attach transactions")?,
        }

        Ok(())
    }

    /// Clean up transactions that are _outdated_ and all transactions that are related to them.
    async fn handle_cleanup(&mut self) -> eyre::Result<()> {
        let now = SystemTime::now();

        let mut outdated_txs = Vec::new();

        for (txid, (_, created_at)) in self.stored_txs.iter() {
            let since_created_at = now
                .duration_since(*created_at)
                .wrap_err("failed to calculate duration since")?;

            if since_created_at > self.tx_outdated_duration {
                outdated_txs.push(*txid);
            }
        }

        for txid in outdated_txs {
            tracing::debug!("Tx {} is outdated", txid);
            self.remove_outdated_tx(txid).await?;
        }

        Ok(())
    }

    /// Remove outdated transaction from storage and all transactions that are related to it.
    async fn remove_outdated_tx(&mut self, txid: Txid) -> eyre::Result<()> {
        let mut txs_to_remove = vec![txid];

        let mut removed_txs_set = HashSet::<Txid>::new();
        removed_txs_set.insert(txid);

        while !txs_to_remove.is_empty() {
            let txid = txs_to_remove.remove(0);

            self.stored_txs.remove(&txid);
            self.remove_tx_from_deps(&txid);

            let Some(inverse_deps) = self.inverse_deps.remove(&txid) else {
                continue;
            };

            for inv_dep in inverse_deps {
                if !removed_txs_set.contains(&inv_dep) {
                    txs_to_remove.push(inv_dep);
                    removed_txs_set.insert(inv_dep);
                }
            }
        }

        Ok(())
    }

    /// Remove tx from all inverse deps. If there is no inverse deps left, then remove it.
    fn remove_tx_from_deps(&mut self, txid: &Txid) {
        let Some(deps) = self.deps.remove(txid) else {
            return;
        };

        for dep in deps {
            let Some(inverse_deps) = self.inverse_deps.get_mut(&dep) else {
                continue;
            };

            inverse_deps.remove(txid);
            if inverse_deps.is_empty() {
                self.inverse_deps.remove(&dep);
            }
        }
    }

    /// Accepts part of the graph of transactions, and attaches them if can.
    ///
    /// If transaction can't be attached, because lack of info (no parent txs),
    /// [`GraphBuilder`] stores them in temporary storage, and waits for them
    /// in next calls of this method.
    ///
    /// If transaction can be attached, then it is stored in [`TransactionsStorage`].
    pub async fn attach_txs(&mut self, checked_txs: &[YuvTransaction]) -> eyre::Result<()> {
        let mut queued_txs = HashSet::new();
        let mut attached_txs = Vec::new();

        for yuv_tx in checked_txs {
            let child_id = yuv_tx.bitcoin_tx.txid();

            match &yuv_tx.tx_type {
                // if issuance or freeze is attached, there is no reason to wait for it's parents.
                YuvTxType::Issue { .. } => {
                    self.set_tx_attached(yuv_tx.clone(), &mut attached_txs)
                        .await?;

                    let Some(ids) = self.inverse_deps.remove(&child_id) else {
                        continue;
                    };

                    // Add to queue for next iteration of graph builder.
                    queued_txs.extend(ids);
                }
                YuvTxType::FreezeToggle { .. } => {
                    self.set_tx_attached(yuv_tx.clone(), &mut attached_txs)
                        .await?;
                }
                YuvTxType::Transfer { input_proofs, .. } => {
                    self.handle_transfer(
                        input_proofs,
                        yuv_tx,
                        child_id,
                        &mut queued_txs,
                        &mut attached_txs,
                    )
                    .await
                    .wrap_err("Failed handling of transfer")?;
                }
            }
        }

        // Attach transactions until there is nothing to do:
        while !queued_txs.is_empty() {
            let mut local_queue = HashSet::new();

            for txid in queued_txs {
                // Find deps of current node that are attached:
                let is_empty = self.remove_attached_parents(txid).await?;

                // If we still dependent on some transactions, then we can't attach this tx.
                if !is_empty {
                    continue;
                }

                // Remove from locally stored txs, and deps:
                let Some((tx, _)) = self.stored_txs.remove(&txid) else {
                    debug_assert!(
                        false,
                        "All parents are attached, but no tx found for {}",
                        txid
                    );
                    continue;
                };
                self.deps.remove(&txid);

                // Add tx to attached storage:
                self.set_tx_attached(tx, &mut attached_txs).await?;

                // Add transactions that depends on this transaction to the queue,
                // so we can remove their deps on next iteration:
                let Some(inv_deps) = self.inverse_deps.remove(&txid) else {
                    continue;
                };

                local_queue.extend(inv_deps);
            }

            queued_txs = local_queue;
        }

        self.handle_fully_attached_txs(attached_txs).await?;

        Ok(())
    }

    /// Handle fully validated transactions, add them to pagination storage and
    /// send event about verified transactions to message handler.
    async fn handle_fully_attached_txs(&mut self, attached_txs: Vec<Txid>) -> eyre::Result<()> {
        if attached_txs.is_empty() {
            return Ok(());
        }

        // Handle that number of transactions in batch could be more than
        // a number of transactions in page.
        for txs in attached_txs.chunks(self.tx_per_page as usize) {
            self.put_txs_ids_to_page(txs)
                .await
                .wrap_err("Failed to store transactions in pages")?;
        }

        self.event_bus
            .send(ControllerMessage::AttachedTxs(attached_txs))
            .await;

        Ok(())
    }

    /// Put attached transactions ids to page storage.
    async fn put_txs_ids_to_page(&self, txids: &[Txid]) -> eyre::Result<()> {
        let last_page_num = self
            .tx_storage
            .get_pages_number()
            .await?
            .unwrap_or_default();

        let mut last_page = self
            .tx_storage
            .get_page_by_num(last_page_num)
            .await?
            .unwrap_or_default();

        // Get space that is left in current page
        let left_space = self.tx_per_page.saturating_sub(last_page.len() as u64);

        // And split attached txs into two arrays, where the first ones will
        // be stored to current page, and other in next.
        let (in_current_page, in_next_page) = split_at(txids, left_space as usize);

        // If there is some left space to store in current page, store it:
        if !in_current_page.is_empty() {
            last_page.extend(in_current_page);

            self.tx_storage.put_page(last_page_num, last_page).await?;
        }

        // If there is some, store them in next page, and increment the page number.
        if !in_next_page.is_empty() {
            let next_page_num = last_page_num + 1;

            self.tx_storage
                .put_page(next_page_num, in_next_page.to_vec())
                .await?;

            self.tx_storage.put_pages_number(next_page_num).await?;
        }

        Ok(())
    }

    /// Removes attached parents from dependencies of the transaction, returns
    /// `true` if there is no deps left.
    async fn remove_attached_parents(&mut self, txid: Txid) -> eyre::Result<bool> {
        let Some(ids) = self.deps.get_mut(&txid) else {
            return Ok(true);
        };

        let mut ids_to_remove = Vec::new();

        // TODO: this could be done in batch with array of futures, but
        // it's not critical for now.
        for id in ids.iter() {
            let is_attached = self.tx_storage.get_yuv_tx(*id).await?.is_some();

            if is_attached {
                ids_to_remove.push(*id);
            }
        }

        for id in ids_to_remove {
            ids.remove(&id);
        }

        Ok(ids.is_empty())
    }

    /// Handle transfer transactions by it's elements (inputs and outputs) to
    /// plain, and inverse dependencies between them.
    ///
    /// If parent of the current tx is attached, skip adding to deps, if all
    /// are attached, then attach current transaction too.
    async fn handle_transfer(
        &mut self,
        input_proofs: &ProofMap,
        yuv_tx: &YuvTransaction,
        child_id: Txid,
        queued_txs: &mut HashSet<Txid>,
        attached_txs: &mut Vec<Txid>,
    ) -> eyre::Result<()> {
        for input in input_proofs.keys() {
            let Some(parent) = yuv_tx.bitcoin_tx.input.get(*input as usize) else {
                debug_assert!(false, "Output proof index is out of bounds");
                continue;
            };

            let parent_id = parent.previous_output.txid;

            let is_attached = self.tx_storage.get_yuv_tx(parent_id).await?.is_some();

            if !is_attached {
                // If there is no parent transaction in the storage, then
                // we need to find it in checked txs or wait for it (add to storage).
                self.inverse_deps
                    .entry(parent_id)
                    .or_default()
                    .insert(child_id);

                self.deps.entry(child_id).or_default().insert(parent_id);
            }
        }

        // May be, we already removed all deps that are attached, so we can check if we can add child
        let all_parents_attached = self.deps.entry(child_id).or_default().is_empty();

        if all_parents_attached {
            // If all parents are attached, then we can attach this transaction.
            self.set_tx_attached(yuv_tx.clone(), attached_txs).await?;

            self.deps.remove(&child_id);

            let Some(ids) = self.inverse_deps.remove(&child_id) else {
                // no reason to add to queue, as there is no deps.
                return Ok(());
            };

            // Add to queue for next iteration of graph builder.
            queued_txs.extend(ids);

            return Ok(());
        }

        // If not all parents are attached, then we need to wait for them.
        self.stored_txs
            .insert(child_id, (yuv_tx.clone(), SystemTime::now()));

        Ok(())
    }

    /// Add transaction to storage and send it to message handler to update an actual inventory
    async fn set_tx_attached(
        &mut self,
        tx: YuvTransaction,
        attached_txs: &mut Vec<Txid>,
    ) -> eyre::Result<()> {
        let txid = tx.bitcoin_tx.txid();

        tracing::info!("Tx {txid} is attached");

        self.tx_storage.put_yuv_tx(tx.clone()).await?;

        // Skip storing inv for freeze transactions (as they are not broadcasted).
        if let YuvTxType::FreezeToggle { freezes } = &tx.tx_type {
            self.update_freezes(&tx.bitcoin_tx.txid(), freezes).await?;

            return Ok(());
        }

        // Add to inventory only if it's not a freeze transaction.
        attached_txs.push(txid);

        Ok(())
    }

    /// For each freeze toggle, update entry in freeze state storage.
    async fn update_freezes(&self, txid: &Txid, freezes: &[FreezeTxToggle]) -> eyre::Result<()> {
        for freeze in freezes {
            let outpoint = OutPoint::new(freeze.txid, freeze.vout);
            let mut value = self
                .state_storage
                .get_frozen_tx(outpoint)
                .await?
                .unwrap_or_default();

            value.tx_ids.push(*txid);

            tracing::debug!(
                "Freeze toggle for txid={} vout={} is set to {:?}",
                freeze.txid,
                freeze.vout,
                value.tx_ids,
            );

            self.state_storage
                .put_frozen_tx(outpoint, value.tx_ids)
                .await?;
        }

        Ok(())
    }
}

/// Split at array without panic
fn split_at<T>(txids: &[T], left_space: usize) -> (&[T], &[T]) {
    txids.split_at(left_space.min(txids.len()))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, str::FromStr};

    use bitcoin::{
        secp256k1::Secp256k1, PackedLockTime, PrivateKey, PublicKey, Sequence, Transaction, Witness,
    };
    use once_cell::sync::Lazy;
    use yuv_pixels::{Pixel, PixelProof, SigPixelProof};
    use yuv_storage::LevelDB;

    use super::*;

    static DUMMY_PIXEL_PROOF: Lazy<PixelProof> = Lazy::new(|| {
        let seckey = PrivateKey::from_str("L43rfkoMRAznnzbFfCXUauvVEqigmkMYxrRPEy91arnofHEUnGiP")
            .expect("Should be valid");

        let key = PublicKey::from_private_key(&Secp256k1::new(), &seckey);

        PixelProof::Sig(SigPixelProof::new(Pixel::new(10, key), key.inner))
    });

    const TX_PER_PAGE: u64 = 100;

    #[tokio::test]
    async fn test_example_from_doc() {
        let storage = LevelDB::in_memory().unwrap();

        let mut event_bus = EventBus::default();
        event_bus.register::<GraphBuilderMessage>(Some(100));
        event_bus.register::<ControllerMessage>(Some(100));

        let mut graph_builder =
            GraphBuilder::<_, _>::new(storage.clone(), storage.clone(), &event_bus, TX_PER_PAGE);

        let tx1 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 1,
                lock_time: PackedLockTime::ZERO,
                input: vec![],
                output: vec![],
            },

            tx_type: YuvTxType::default(),
        };
        dbg!(tx1.bitcoin_tx.txid());

        let tx2 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 2,
                lock_time: PackedLockTime::ZERO,
                input: vec![],
                output: vec![],
            },

            tx_type: YuvTxType::default(),
        };
        dbg!(tx2.bitcoin_tx.txid());

        let tx6 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 3,
                lock_time: PackedLockTime::ZERO,
                input: vec![],
                output: vec![],
            },

            tx_type: YuvTxType::default(),
        };
        dbg!(tx6.bitcoin_tx.txid());

        storage.put_yuv_tx(tx1.clone()).await.unwrap();
        storage.put_yuv_tx(tx2.clone()).await.unwrap();
        storage.put_yuv_tx(tx6.clone()).await.unwrap();

        let tx3 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 4,
                lock_time: PackedLockTime::ZERO,
                input: vec![
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx1.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx2.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                ],
                output: vec![],
            },

            tx_type: YuvTxType::Transfer {
                input_proofs: {
                    let mut map = BTreeMap::new();

                    map.insert(0, DUMMY_PIXEL_PROOF.clone());
                    map.insert(1, DUMMY_PIXEL_PROOF.clone());

                    map
                },
                output_proofs: Default::default(),
            },
        };
        dbg!(tx3.bitcoin_tx.txid());

        let tx7 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 5,
                lock_time: PackedLockTime::ZERO,
                input: vec![],
                output: vec![],
            },

            tx_type: YuvTxType::default(),
        };
        dbg!(tx7.bitcoin_tx.txid());

        let tx4 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 6,
                lock_time: PackedLockTime::ZERO,
                input: vec![
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx3.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx7.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx6.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                ],
                output: vec![],
            },

            tx_type: YuvTxType::Transfer {
                input_proofs: {
                    let mut map = BTreeMap::new();

                    map.insert(0, DUMMY_PIXEL_PROOF.clone());
                    map.insert(1, DUMMY_PIXEL_PROOF.clone());
                    map.insert(2, DUMMY_PIXEL_PROOF.clone());

                    map
                },
                output_proofs: Default::default(),
            },
        };
        dbg!(tx4.bitcoin_tx.txid());

        let tx5 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 7,
                lock_time: PackedLockTime::ZERO,
                input: vec![bitcoin::TxIn {
                    previous_output: bitcoin::OutPoint::new(tx4.bitcoin_tx.txid(), 0),
                    script_sig: bitcoin::Script::default(),
                    sequence: Sequence(0),
                    witness: Witness::default(),
                }],
                output: vec![],
            },

            tx_type: YuvTxType::Transfer {
                input_proofs: {
                    let mut map = BTreeMap::new();

                    map.insert(0, DUMMY_PIXEL_PROOF.clone());

                    map
                },
                output_proofs: Default::default(),
            },
        };
        dbg!(tx5.bitcoin_tx.txid());

        let txs = vec![tx5.clone(), tx4.clone(), tx3.clone(), tx7.clone()];

        graph_builder.attach_txs(&txs).await.unwrap();

        for tx in &txs {
            let got_tx = storage.get_yuv_tx(tx.bitcoin_tx.txid()).await.unwrap();

            assert_eq!(
                got_tx,
                Some(tx.clone()),
                "Transaction {} must be attcahed",
                tx.bitcoin_tx.txid()
            );
        }

        assert!(
            graph_builder.deps.is_empty(),
            "Deps must be empty: {:?}",
            graph_builder.deps
        );
        assert!(
            graph_builder.inverse_deps.is_empty(),
            "Inverse deps must be empty: {:?}",
            graph_builder.inverse_deps
        );
        assert!(graph_builder.stored_txs.is_empty());

        let page = storage
            .get_page_by_num(0)
            .await
            .unwrap()
            .expect("failed getting a page");

        assert_eq!(page.len(), txs.len())
    }

    #[tokio::test]
    async fn test_cleanup() -> eyre::Result<()> {
        let storage = LevelDB::in_memory().unwrap();

        let mut event_bus = EventBus::default();
        event_bus.register::<GraphBuilderMessage>(Some(100));
        event_bus.register::<ControllerMessage>(Some(100));

        let graph_builder =
            GraphBuilder::new(storage.clone(), storage.clone(), &event_bus, TX_PER_PAGE);

        let mut graph_builder = graph_builder
            .with_cleanup_period(Duration::from_secs(0))
            .with_outdated_duration(Duration::from_secs(0));

        let tx1 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 1,
                lock_time: PackedLockTime::ZERO,
                input: vec![],
                output: vec![],
            },

            tx_type: YuvTxType::default(),
        };
        dbg!(tx1.bitcoin_tx.txid());

        let tx2 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 2,
                lock_time: PackedLockTime::ZERO,
                input: vec![],
                output: vec![],
            },

            tx_type: YuvTxType::default(),
        };
        dbg!(tx2.bitcoin_tx.txid());

        let tx6 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 3,
                lock_time: PackedLockTime::ZERO,
                input: vec![],
                output: vec![],
            },

            tx_type: YuvTxType::default(),
        };
        dbg!(tx6.bitcoin_tx.txid());

        let tx3 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 4,
                lock_time: PackedLockTime::ZERO,
                input: vec![
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx1.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx2.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                ],
                output: vec![],
            },

            tx_type: YuvTxType::Transfer {
                input_proofs: {
                    let mut map = BTreeMap::new();

                    map.insert(0, DUMMY_PIXEL_PROOF.clone());
                    map.insert(1, DUMMY_PIXEL_PROOF.clone());

                    map
                },
                output_proofs: Default::default(),
            },
        };
        dbg!(tx3.bitcoin_tx.txid());

        let tx7 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 5,
                lock_time: PackedLockTime::ZERO,
                input: vec![],
                output: vec![],
            },

            tx_type: YuvTxType::default(),
        };
        dbg!(tx7.bitcoin_tx.txid());

        let tx4 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 6,
                lock_time: PackedLockTime::ZERO,
                input: vec![
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx3.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx7.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                    bitcoin::TxIn {
                        previous_output: bitcoin::OutPoint::new(tx6.bitcoin_tx.txid(), 0),
                        script_sig: bitcoin::Script::default(),
                        sequence: Sequence(0),
                        witness: Witness::default(),
                    },
                ],
                output: vec![],
            },

            tx_type: YuvTxType::Transfer {
                input_proofs: {
                    let mut map = BTreeMap::new();

                    map.insert(0, DUMMY_PIXEL_PROOF.clone());
                    map.insert(1, DUMMY_PIXEL_PROOF.clone());
                    map.insert(2, DUMMY_PIXEL_PROOF.clone());

                    map
                },
                output_proofs: Default::default(),
            },
        };
        dbg!(tx4.bitcoin_tx.txid());

        let tx5 = YuvTransaction {
            bitcoin_tx: Transaction {
                version: 7,
                lock_time: PackedLockTime::ZERO,
                input: vec![bitcoin::TxIn {
                    previous_output: bitcoin::OutPoint::new(tx4.bitcoin_tx.txid(), 0),
                    script_sig: bitcoin::Script::default(),
                    sequence: Sequence(0),
                    witness: Witness::default(),
                }],
                output: vec![],
            },

            tx_type: YuvTxType::Transfer {
                input_proofs: {
                    let mut map = BTreeMap::new();

                    map.insert(0, DUMMY_PIXEL_PROOF.clone());

                    map
                },
                output_proofs: Default::default(),
            },
        };
        dbg!(tx5.bitcoin_tx.txid());

        graph_builder
            .attach_txs(&vec![
                tx6.clone(),
                tx2.clone(),
                tx5.clone(),
                tx4.clone(),
                tx3.clone(),
                tx7.clone(),
            ])
            .await?;

        assert!(
            !graph_builder.deps.is_empty(),
            "Deps mustn't be empty before cleaning"
        );
        assert!(
            !graph_builder.inverse_deps.is_empty(),
            "InvDeps mustn't be empty before cleaning"
        );
        assert!(
            !graph_builder.stored_txs.is_empty(),
            "StoredTxs mustn't be empty before cleaning"
        );

        graph_builder.handle_cleanup().await?;

        assert!(
            graph_builder.deps.is_empty(),
            "Deps must be empty after cleaning: {:?}",
            graph_builder.deps
        );
        assert!(
            graph_builder.inverse_deps.is_empty(),
            "InvDeps must be empty after cleaning: {:?}",
            graph_builder.inverse_deps
        );
        assert!(
            graph_builder.stored_txs.is_empty(),
            "StoredTxs must be empty after cleaning: {:?}",
            graph_builder.stored_txs
        );

        Ok(())
    }
}
