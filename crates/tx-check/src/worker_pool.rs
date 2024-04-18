use crate::TxCheckerWorker;

use crate::worker::Config;
use bitcoin_client::{BitcoinRpcApi, Error as BitcoinRpcError};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use yuv_storage::{FrozenTxsStorage, InvalidTxsStorage, TransactionsStorage};

pub struct TxCheckerWorkerPool<TS, SS, BC>
where
    TS: TransactionsStorage + Clone + Send + Sync + 'static,
    SS: InvalidTxsStorage + FrozenTxsStorage + Clone + Send + Sync + 'static,
    BC: BitcoinRpcApi + Send + Sync + 'static,
{
    workers: Vec<TxCheckerWorker<TS, SS, BC>>,
}

impl<TS, SS, BC> TxCheckerWorkerPool<TS, SS, BC>
where
    TS: TransactionsStorage + Clone + Send + Sync + 'static,
    SS: InvalidTxsStorage + FrozenTxsStorage + Clone + Send + Sync + 'static,
    BC: BitcoinRpcApi + Send + Sync + 'static,
{
    pub fn from_config(
        pool_size: usize,
        worker_config: Config<TS, SS, BC>,
    ) -> Result<Self, BitcoinRpcError> {
        let workers = (0..pool_size)
            .map(|i| TxCheckerWorker::from_config(&worker_config, Some(i)))
            .collect::<Vec<TxCheckerWorker<TS, SS, BC>>>();

        Ok(Self { workers })
    }

    pub async fn run(self, cancellation: CancellationToken) {
        let task_tracker = TaskTracker::new();

        for worker in self.workers {
            task_tracker.spawn(worker.run(cancellation.child_token()));
        }

        task_tracker.close();
        task_tracker.wait().await;
    }
}
