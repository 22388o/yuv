use crate::Subindexer;
use async_trait::async_trait;
use bitcoin::Txid;
use bitcoin_client::json::GetBlockTxResult;
use bitcoin_client::BitcoinRpcApi;
use event_bus::{typeid, EventBus, Receiver};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use yuv_types::{ConfirmationIndexerMessage, TxCheckerMessage, YuvTransaction};

/// Is responsible for waiting confirmations of transactions in Bitcoin.
pub struct ConfirmationIndexer<BC>
where
    BC: BitcoinRpcApi + Send + Sync + 'static,
{
    event_bus: EventBus,
    new_tx_rx: Receiver<ConfirmationIndexerMessage>,
    bitcoin_client: Arc<BC>,

    /// Confirmations queue. Contains transactions that are waiting confirmation.
    queue: Arc<Mutex<HashMap<Txid, UnconfirmedTransaction>>>,
    /// Max time that transaction can wait confirmation before it will be removed from the queue.
    max_confirmation_time: Duration,
}

/// Number of confirmations that is required to consider transaction as confirmed.
const MIN_CONFIRMATIONS: u32 = 1;

impl<BC> ConfirmationIndexer<BC>
where
    BC: BitcoinRpcApi + Send + Sync + 'static,
{
    pub fn new(
        full_event_bus: &EventBus,
        bitcoin_client: Arc<BC>,
        max_confirmation_time: Duration,
    ) -> Self {
        let event_bus = full_event_bus
            .extract(
                &typeid![TxCheckerMessage],
                &typeid![ConfirmationIndexerMessage],
            )
            .expect("event channels must be presented");

        Self {
            new_tx_rx: event_bus.subscribe::<ConfirmationIndexerMessage>(),
            event_bus,
            queue: Default::default(),
            max_confirmation_time,
            bitcoin_client,
        }
    }

    async fn handle_event(&mut self, event: ConfirmationIndexerMessage) -> eyre::Result<()> {
        use ConfirmationIndexerMessage as CIM;

        match event {
            CIM::ConfirmBatchTx(yuv_txs) => self.handle_confirm_txs(yuv_txs).await?,
        }

        Ok(())
    }

    async fn handle_confirm_txs(&mut self, yuv_txs: Vec<YuvTransaction>) -> eyre::Result<()> {
        for yuv_tx in yuv_txs {
            self.handle_confirm_tx(yuv_tx).await?;
        }

        Ok(())
    }

    /// Handle new transaction to confirm it. If transaction is already confirmed, then it will be
    /// sent to the `TxChecker`. Otherwise it will be added to the queue.
    async fn handle_confirm_tx(&mut self, yuv_tx: YuvTransaction) -> eyre::Result<()> {
        let mut queue = self.queue.lock().await;

        let got_tx = self
            .bitcoin_client
            .get_raw_transaction_info(&yuv_tx.bitcoin_tx.txid(), None)
            .await?;

        if let Some(confirmations) = got_tx.confirmations {
            if confirmations >= MIN_CONFIRMATIONS {
                self.new_confirmed_tx(yuv_tx).await;
                return Ok(());
            }

            tracing::debug!("confirmations too low: {}", confirmations);
        }

        tracing::debug!(
            "Received new transaction to confirm: {:?}",
            yuv_tx.bitcoin_tx.txid()
        );

        queue.insert(
            yuv_tx.bitcoin_tx.txid(),
            UnconfirmedTransaction {
                yuv_tx,
                created_at: SystemTime::now(),
            },
        );

        Ok(())
    }

    /// Find transactions that are waiting confirmation in the block. If transaction is appeared in
    /// the block, then it is confirmed and can be sent to the checkers. Otherwise it will be
    /// removed from the queue if it is waiting confirmation for too long.
    pub async fn find_waiting_txs(&mut self, block: &GetBlockTxResult) -> eyre::Result<()> {
        let new_events_num = self.new_tx_rx.len();
        for _ in 0..new_events_num {
            let new_event = self.new_tx_rx.recv().await.expect("channel must be alive");
            self.handle_event(new_event).await?;
        }

        let mut queue = self.queue.lock().await;
        if queue.is_empty() {
            return Ok(());
        }

        // If transaction is appeared in the block, then it is confirmed and can be sent to the
        // `TxChecker`.
        for tx in block.tx.iter() {
            if let Some(unconfirmed_tx) = queue.remove(&tx.txid()) {
                self.new_confirmed_tx(unconfirmed_tx.yuv_tx).await;
            }
        }

        // Remove transactions that are waiting confirmation for too long.
        for (txid, unconfirmed_tx) in queue.clone().iter() {
            if unconfirmed_tx.created_at.elapsed().unwrap() > self.max_confirmation_time {
                tracing::debug!(
                    "Transaction {:?} is waiting confirmation for too long. Removing from queue.",
                    txid
                );

                queue.remove(txid);
            }
        }

        Ok(())
    }

    async fn new_confirmed_tx(&self, yuv_tx: YuvTransaction) {
        tracing::debug!("Transaction confirmed: {:?}", yuv_tx.bitcoin_tx.txid());

        self.event_bus
            .send(TxCheckerMessage::NewTxs {
                txs: vec![yuv_tx],
                sender: None,
            })
            .await;
    }
}

#[async_trait]
impl<BC> Subindexer for ConfirmationIndexer<BC>
where
    BC: BitcoinRpcApi + Send + Sync + 'static,
{
    async fn index(&mut self, block: &GetBlockTxResult) -> eyre::Result<()> {
        self.find_waiting_txs(block).await
    }
}

/// Transaction that is waiting confirmation. Contains timestamp of creation and transaction itself.
/// Timestamp is used to check that transaction is waiting confirmation for too long (considering
/// several days) and should be removed from the queue.
#[derive(Clone)]
struct UnconfirmedTransaction {
    pub created_at: SystemTime,
    pub yuv_tx: YuvTransaction,
}
