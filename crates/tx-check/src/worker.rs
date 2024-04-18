use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

use bitcoin::{OutPoint, Txid};
use bitcoin_client::BitcoinRpcApi;
use event_bus::{typeid, EventBus};
use eyre::{eyre, Context, Result};
use tokio_util::sync::CancellationToken;

use yuv_pixels::PixelProof;
use yuv_storage::{FrozenTxsStorage, InvalidTxsStorage, TransactionsStorage};
use yuv_types::messages::p2p::Inventory;
use yuv_types::{
    ControllerMessage, FreezeTxToggle, GraphBuilderMessage, ProofMap, TxCheckerMessage,
    YuvTransaction, YuvTxType,
};

use crate::transaction::find_issuer_in_txinputs;
use crate::{
    errors::{CheckError, TxCheckerError},
    transaction::check_transaction,
};

pub struct Config<TxsStorage, StateStorage, BitcoinClient>
where
    StateStorage: InvalidTxsStorage + FrozenTxsStorage + Send + Sync + 'static,
    TxsStorage: TransactionsStorage + Send + Sync + 'static,
    BitcoinClient: BitcoinRpcApi + Send + Sync + 'static,
{
    pub full_event_bus: EventBus,
    pub bitcoin_client: Arc<BitcoinClient>,
    pub txs_storage: TxsStorage,
    pub state_storage: StateStorage,
}

/// Async implementation of [`TxChecker`] for node implementation.
///
/// Accepts [`YuvTransaction`]s from channel, check them and sends to graph builder.
///
/// [`TxChecker`]: struct.TxChecker.html
pub struct TxCheckerWorker<TxsStorage, StateStorage, BitcoinClient>
where
    StateStorage: InvalidTxsStorage + FrozenTxsStorage + Send + Sync + 'static,
    TxsStorage: TransactionsStorage + Send + Sync + 'static,
    BitcoinClient: BitcoinRpcApi + Send + Sync + 'static,
{
    /// Index of the worker in the worker pool
    index: usize,

    /// Bitcoin RPC client to check that transaction exists.
    bitcoin_client: Arc<BitcoinClient>,

    /// Inner storage of already checked and attached transactions.
    txs_storage: TxsStorage,

    /// Storage for inner states of transactions.
    state_storage: StateStorage,

    /// Event bus for simplifying communication with services
    event_bus: EventBus,
}

impl<TS, SS, BC> TxCheckerWorker<TS, SS, BC>
where
    TS: TransactionsStorage + Clone + Send + Sync + 'static,
    SS: InvalidTxsStorage + Clone + FrozenTxsStorage + Send + Sync + 'static,
    BC: BitcoinRpcApi + Send + Sync + 'static,
{
    pub fn from_config(config: &Config<TS, SS, BC>, index: Option<usize>) -> Self {
        let event_bus = config
            .full_event_bus
            .extract(
                &typeid![GraphBuilderMessage, ControllerMessage],
                &typeid![TxCheckerMessage],
            )
            .expect("event channels must be presented");

        Self {
            index: index.unwrap_or_default(),
            event_bus,
            bitcoin_client: config.bitcoin_client.clone(),
            txs_storage: config.txs_storage.clone(),
            state_storage: config.state_storage.clone(),
        }
    }

    pub async fn run(mut self, cancellation: CancellationToken) {
        let events = self.event_bus.subscribe::<TxCheckerMessage>();

        loop {
            tokio::select! {
                event_received = events.recv() => {
                    let Ok(event) = event_received else {
                        tracing::trace!(index = self.index, "All incoming events senders are dropped");
                        return;
                    };

                    if let Err(err) = self.handle_event(event).await {
                        tracing::error!(index = self.index, "Failed to handle an event: {}", err);
                    }
                }
                _ = cancellation.cancelled() => {
                    tracing::trace!(index = self.index, "Cancellation received, stopping TxCheckerWorker");
                    return;
                }
            }
        }
    }

    async fn handle_event(&mut self, event: TxCheckerMessage) -> Result<()> {
        match event {
            TxCheckerMessage::NewTxs { txs, sender } => self
                .check(txs, sender)
                .await
                .wrap_err("failed to check transactions")?,
        }

        Ok(())
    }

    pub async fn check(
        &mut self,
        txs: Vec<YuvTransaction>,
        peer_addr: Option<SocketAddr>,
    ) -> Result<()> {
        let mut checked_txs = BTreeMap::new();
        let mut invalid_txs = Vec::new();
        let mut not_found_parents = Vec::new();

        'outer: for tx in txs {
            if let Err(err) = self.check_tx(&tx).await {
                tracing::info!(
                    index = self.index,
                    "Received an invalid transaction {}: {}",
                    tx.bitcoin_tx.txid(),
                    err.to_string(),
                );

                invalid_txs.push(tx);

                continue;
            }

            // Gather parent, that are still not in the storage nor in the current batch:
            match &tx.tx_type {
                // Issue transactions have no inputs:
                YuvTxType::Issue { .. } => {}
                YuvTxType::FreezeToggle { freezes } => {
                    for FreezeTxToggle { txid, vout } in freezes {
                        let Some(yuv_tx) = self.txs_storage.get_yuv_tx(*txid).await? else {
                            // If there is no transactions, worker will wait its appearance for
                            // check.
                            continue;
                        };

                        let output_proofs = match get_output_proofs(&yuv_tx) {
                            Some(value) => value,
                            None => {
                                // TODO: Even if issuer is in input, that's strange that he
                                // tries to freeze output of a freeze tx. So worker will just ignore
                                // it for now.
                                tracing::warn!(
                                    index = self.index,
                                    "Freeze tx {} tries to freeze a freeze tx {}",
                                    txid,
                                    tx.bitcoin_tx.txid(),
                                );
                                continue;
                            }
                        };

                        let Some(output) = output_proofs.get(vout) else {
                            tracing::info!(
                                index = self.index,
                                "Freeze tx {} is invalid: output {} not found",
                                txid,
                                vout,
                            );

                            // TODO: should we consider this transaction fully
                            // invalid or just ignore this output?
                            continue 'outer;
                        };
                        let chroma = &output.pixel().chroma;

                        // Check signer of the freeze tx is issuer of the chroma
                        // which frozen tx has:
                        if find_issuer_in_txinputs(&tx.bitcoin_tx.input, chroma).is_none() {
                            tracing::info!(
                                index = self.index,
                                "Freeze tx {} is invalid: none of the inputs has issuer, removing it",
                                txid,
                            );

                            // Remove invalid freeze tx from storage:
                            self.txs_storage.delete_yuv_tx(*txid).await?;

                            continue 'outer;
                        }
                    }
                }
                // Transfer has inputs:
                YuvTxType::Transfer {
                    ref input_proofs, ..
                } => {
                    self.check_transfer(
                        &tx,
                        input_proofs,
                        &checked_txs,
                        &mut invalid_txs,
                        &mut not_found_parents,
                    )
                    .await?;
                }
            }

            checked_txs.insert(tx.bitcoin_tx.txid(), tx);
        }

        // Send checked transactions to next worker:
        if !checked_txs.is_empty() {
            self.event_bus
                .send(GraphBuilderMessage::CheckedTxs(
                    checked_txs.values().cloned().collect::<Vec<_>>(),
                ))
                .await;
        }

        // Notify about invalid transactions:
        if !invalid_txs.is_empty() {
            let invalid_txs_ids = invalid_txs.iter().map(|tx| tx.bitcoin_tx.txid()).collect();
            self.event_bus
                .send(ControllerMessage::InvalidTxs {
                    tx_ids: invalid_txs_ids,
                    sender: peer_addr,
                })
                .await;

            self.state_storage.put_invalid_txs(invalid_txs).await?;
        }

        // If there is no info about parent transactions, request them:
        if !not_found_parents.is_empty() {
            if let Some(addr) = peer_addr {
                self.event_bus
                    .send(ControllerMessage::GetData {
                        inv: not_found_parents
                            .iter()
                            .map(|txid| Inventory::Ytx(*txid))
                            .collect(),
                        receiver: addr,
                    })
                    .await;
            }
        }

        Ok(())
    }

    async fn check_transfer(
        &mut self,
        tx: &YuvTransaction,
        input_proofs: &ProofMap,
        checked_txs: &BTreeMap<Txid, YuvTransaction>,
        invalid_txs: &mut Vec<YuvTransaction>,
        not_found_parents: &mut Vec<Txid>,
    ) -> Result<()> {
        for (parent, proof) in input_proofs {
            let Some(parent_tx) = tx.bitcoin_tx.input.get(*parent as usize) else {
                return Err(CheckError::InputNotFound.into());
            };

            let OutPoint { txid, vout } = parent_tx.previous_output;

            if self.is_tx_frozen(&txid, vout, proof).await? {
                tracing::info!(
                    index = self.index,
                    "Transfer tx {} is invalid: output {} of tx {} is frozen",
                    tx.bitcoin_tx.txid(),
                    vout,
                    txid,
                );

                invalid_txs.push(tx.clone());
                return Ok(());
            }

            let is_in_storage = self.txs_storage.get_yuv_tx(txid).await?.is_some();

            if !is_in_storage && !checked_txs.contains_key(&txid) {
                not_found_parents.push(txid);
            }
        }
        Ok(())
    }

    /// Check if transaction is frozen
    async fn is_tx_frozen(&self, txid: &Txid, vout: u32, proof: &PixelProof) -> Result<bool> {
        let outpoint = OutPoint::new(*txid, vout);
        let freeze_entry = self.state_storage.get_frozen_tx(outpoint).await?;

        // Issuer haven't attempted to freeze this output, so it's not frozen:
        let Some(freeze_entry) = freeze_entry else {
            return Ok(false);
        };

        let mut checked_freezes = Vec::new();

        // TODO: optimize this approach.
        for freeze_txid in freeze_entry.tx_ids {
            let freeze_tx = self
                .txs_storage
                .get_yuv_tx(freeze_txid)
                .await?
                .ok_or_else(|| eyre!("Freeze tx not found, {}", freeze_txid))?;

            if find_issuer_in_txinputs(&freeze_tx.bitcoin_tx.input, &proof.pixel().chroma).is_none()
            {
                tracing::info!(
                    index = self.index,
                    "Freeze tx {} is invalid: none of the inputs has issuer, removing it",
                    freeze_txid,
                );

                self.txs_storage.delete_yuv_tx(freeze_txid).await?;

                continue;
            }

            checked_freezes.push(freeze_txid);
        }

        let is_frozen = checked_freezes.len() % 2 == 1;

        self.state_storage
            .put_frozen_tx(outpoint, checked_freezes)
            .await?;

        Ok(is_frozen)
    }

    async fn check_tx(&mut self, yuv_tx: &YuvTransaction) -> Result<(), TxCheckerError> {
        // FIXME: for now we set as invalid transactions which failed in process of
        // getting one from network (node). In future, we should check if it's problems with
        // network or transaction is really invalid.
        let got_tx = self
            .bitcoin_client
            .get_raw_transaction(&yuv_tx.bitcoin_tx.txid(), None)
            .await?;

        if got_tx != yuv_tx.bitcoin_tx {
            return Err(TxCheckerError::TransactionMismatch);
        }

        check_transaction(yuv_tx)?;

        Ok(())
    }
}

fn get_output_proofs(yuv_tx: &YuvTransaction) -> Option<&BTreeMap<u32, PixelProof>> {
    match yuv_tx.tx_type {
        YuvTxType::Issue { ref output_proofs } => Some(output_proofs),
        YuvTxType::Transfer {
            ref output_proofs, ..
        } => Some(output_proofs),
        YuvTxType::FreezeToggle { .. } => None,
    }
}
