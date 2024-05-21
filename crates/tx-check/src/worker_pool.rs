use crate::TxCheckerWorker;

use crate::worker::Config;
use bitcoin_client::Error as BitcoinRpcError;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use yuv_storage::{ChromaInfoStorage, FrozenTxsStorage, InvalidTxsStorage, TransactionsStorage};

pub struct TxCheckerWorkerPool<TransactoinsStorage, StateStorage> {
    workers: Vec<TxCheckerWorker<TransactoinsStorage, StateStorage>>,
}

impl<TS, SS> TxCheckerWorkerPool<TS, SS>
where
    TS: TransactionsStorage + Clone + Send + Sync + 'static,
    SS: InvalidTxsStorage + FrozenTxsStorage + ChromaInfoStorage + Clone + Send + Sync + 'static,
{
    pub fn from_config(
        pool_size: usize,
        worker_config: Config<TS, SS>,
    ) -> Result<Self, BitcoinRpcError> {
        let workers = (0..pool_size)
            .map(|i| TxCheckerWorker::from_config(&worker_config, Some(i)))
            .collect::<Vec<TxCheckerWorker<TS, SS>>>();

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
