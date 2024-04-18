use std::sync::Arc;
use std::time::Duration;

use bitcoin_client::BitcoinRpcClient;
use event_bus::EventBus;
use eyre::Context;
use tokio::select;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::info;

use yuv_controller::Controller;
use yuv_indexers::{BitcoinBlockIndexer, ConfirmationIndexer, FreezesIndexer, RunParams};

use yuv_p2p::{
    client::{Handle, P2PClient},
    net::{ReactorTcp, Waker},
};
use yuv_rpc_server::ServerConfig;
use yuv_storage::{FlushStrategy, LevelDB, LevelDbOptions, TxStatesStorage};
use yuv_tx_attach::GraphBuilder;
use yuv_tx_check::{Config as CheckerConfig, TxCheckerWorkerPool};
use yuv_types::{
    ConfirmationIndexerMessage, ControllerMessage, GraphBuilderMessage, TxCheckerMessage,
};

use crate::config::{NodeConfig, StorageConfig};

const DEFAULT_CHANNEL_SIZE: usize = 1000;

const DEFAULT_SHUTDOWN_TIMEOUT_SECS: u64 = 30;

/// Node encapsulate node service's start
pub struct Node {
    config: NodeConfig,
    event_bus: EventBus,
    txs_storage: LevelDB,
    state_storage: LevelDB,
    txs_states_storage: TxStatesStorage,
    btc_client: Arc<BitcoinRpcClient>,

    cancelation: CancellationToken,
    task_tracker: TaskTracker,
}

impl Node {
    pub async fn new(config: NodeConfig) -> eyre::Result<Self> {
        let event_bus = Self::init_event_bus();
        let (txs_storage, state_storage) = Self::init_storage(config.storage.clone())?;
        let tx_states_storage = TxStatesStorage::default();

        let btc_client = Arc::new(
            BitcoinRpcClient::new(config.bnode.auth().clone(), config.bnode.url.clone()).await?,
        );

        Ok(Self {
            config,
            event_bus,
            txs_storage,
            state_storage,
            txs_states_storage: tx_states_storage,
            btc_client,
            cancelation: CancellationToken::new(),
            task_tracker: TaskTracker::new(),
        })
    }

    /// The order of service starting is important if you want to index blocks first and then start
    /// listen to inbound messages.
    pub async fn run(&self) -> eyre::Result<()> {
        self.spawn_graph_builder();
        self.spawn_tx_checkers_worker_pool()?;
        self.spawn_indexer().await?;

        let p2p_handle = self.spawn_p2p()?;
        self.spawn_controller(p2p_handle);

        self.spawn_rpc();

        self.task_tracker.close();

        Ok(())
    }

    fn spawn_p2p(&self) -> eyre::Result<Handle<Waker>> {
        let p2p_client_runner =
            P2PClient::<ReactorTcp>::new(self.config.p2p.clone().try_into()?, &self.event_bus)
                .expect("P2P client must be successfully created");

        let handle = p2p_client_runner.handle();

        self.task_tracker
            .spawn(p2p_client_runner.run(self.cancelation.clone()));

        Ok(handle)
    }

    fn spawn_controller(&self, handle: Handle<Waker>) {
        let controller = Controller::new(
            &self.event_bus,
            self.txs_storage.clone(),
            self.state_storage.clone(),
            self.txs_states_storage.clone(),
            handle,
        )
        .set_inv_sharing_interval(Duration::from_secs(
            self.config.controller.inv_sharing_interval,
        ))
        .set_max_inv_size(self.config.controller.max_inv_size);

        self.task_tracker
            .spawn(controller.run(self.cancelation.clone()));
    }

    fn spawn_graph_builder(&self) {
        let graph_builder = GraphBuilder::new(
            self.txs_storage.clone(),
            self.state_storage.clone(),
            &self.event_bus,
            self.config.storage.tx_per_page,
        );

        self.task_tracker
            .spawn(graph_builder.run(self.cancelation.clone()));
    }

    fn spawn_tx_checkers_worker_pool(&self) -> eyre::Result<()> {
        let worker_pool = TxCheckerWorkerPool::from_config(
            self.config.checkers.pool_size,
            CheckerConfig {
                full_event_bus: self.event_bus.clone(),
                bitcoin_client: self.btc_client.clone(),
                txs_storage: self.txs_storage.clone(),
                state_storage: self.state_storage.clone(),
            },
        )
        .wrap_err("TxCheckers worker pool must run successfully")?;

        self.task_tracker
            .spawn(worker_pool.run(self.cancelation.clone()));

        Ok(())
    }

    fn spawn_rpc(&self) {
        let address = self.config.rpc.address.to_string();
        let max_items_per_request = self.config.rpc.max_items_per_request;

        self.task_tracker.spawn(yuv_rpc_server::run_server(
            ServerConfig {
                address,
                max_items_per_request,
            },
            self.txs_storage.clone(),
            self.state_storage.clone(),
            self.event_bus.clone(),
            self.txs_states_storage.clone(),
            self.btc_client.clone(),
            self.cancelation.clone(),
        ));
    }

    async fn spawn_indexer(&self) -> eyre::Result<()> {
        let mut indexer =
            BitcoinBlockIndexer::new(self.btc_client.clone(), self.state_storage.clone());

        indexer.add_indexer(FreezesIndexer::new(&self.event_bus));
        indexer.add_indexer(ConfirmationIndexer::new(
            &self.event_bus,
            self.btc_client.clone(),
            self.config.indexer.max_confirmation_time,
        ));

        indexer
            .init(self.config.indexer.clone().into())
            .await
            .wrap_err("failed to initialize indexer")?;

        self.task_tracker.spawn(indexer.run(
            RunParams {
                polling_period: self.config.indexer.polling_period,
            },
            self.cancelation.clone(),
        ));

        Ok(())
    }

    fn init_storage(config: StorageConfig) -> eyre::Result<(LevelDB, LevelDB)> {
        // Create directory if it does not exist
        if !config.path.exists() {
            std::fs::create_dir_all(&config.path)
                .wrap_err_with(|| format!("failed to create directory {:?}", config.path))?;
        }

        // Initialize storage for transactions
        let opt = LevelDbOptions {
            create_if_missing: config.create_if_missing,
            path: config.path.join("transactions"),
            flush_strategy: FlushStrategy::Ticker {
                period: config.flush_period,
            },
        };
        let txs_storage = LevelDB::from_opts(opt).wrap_err("failed to initialize storage")?;

        // Initialize storage for states
        let opt = LevelDbOptions {
            path: config.path.join("state"),
            create_if_missing: config.create_if_missing,
            flush_strategy: FlushStrategy::Ticker {
                period: config.flush_period,
            },
        };
        let state_storage = LevelDB::from_opts(opt).wrap_err("failed to initialize storage")?;

        Ok((txs_storage, state_storage))
    }

    fn init_event_bus() -> EventBus {
        let mut event_bus = EventBus::default();
        event_bus.register::<TxCheckerMessage>(Some(DEFAULT_CHANNEL_SIZE));
        event_bus.register::<GraphBuilderMessage>(Some(DEFAULT_CHANNEL_SIZE));
        event_bus.register::<ControllerMessage>(Some(DEFAULT_CHANNEL_SIZE));
        event_bus.register::<ConfirmationIndexerMessage>(Some(DEFAULT_CHANNEL_SIZE));

        event_bus
    }

    pub async fn shutdown(&self) {
        info!("Shutting down node, finishing received requests...");

        self.cancelation.cancel();

        let timeout = self
            .config
            .shutdown_timeout
            .unwrap_or(DEFAULT_SHUTDOWN_TIMEOUT_SECS);

        select! {
            // Wait until all tasks are finished
            _ = self.task_tracker.wait() => {},
            // Or wait for and exit by timeout
            _ = sleep(Duration::from_secs(timeout)) => {
                tracing::info!("Shutdown timeout reached, exiting...");
            },
        }
    }
}