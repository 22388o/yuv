use std::sync::Arc;

use bitcoin_client::BitcoinRpcClient;
use event_bus::EventBus;
use jsonrpsee::server::Server;
use tokio_util::sync::CancellationToken;

use yuv_rpc_api::transactions::YuvTransactionsRpcServer;
use yuv_storage::{
    ChromaInfoStorage, FrozenTxsStorage, PagesStorage, TransactionsStorage, TxStatesStorage,
};

use crate::transactions::TransactionsController;

pub mod transactions;

/// The average YUV tx size in bytes
///
/// Includes average Bitcoin tx size (300-400 bytes) and average number
/// of proofs for inputs and outputs (3-6) with their sizes (64 bytes for
/// single sig proof of each input and output). Rounded to 1 kilobyte.
const AVERAGE_TX_SIZE: usize = 1024;

pub struct ServerConfig {
    /// Address at which the server will listen for incoming connections.
    pub address: String,
    /// Max number of items to request/process per incoming request.
    pub max_items_per_request: usize,
}

/// Runs YUV Node's RPC server.
pub async fn run_server<S, AS>(
    ServerConfig {
        address,
        max_items_per_request,
    }: ServerConfig,
    txs_storage: S,
    frozen_storage: AS,
    full_event_bus: EventBus,
    txs_states_storage: TxStatesStorage,
    bitcoin_client: Arc<BitcoinRpcClient>,
    cancellation: CancellationToken,
) -> eyre::Result<()>
where
    S: TransactionsStorage + PagesStorage + Clone + Send + Sync + 'static,
    AS: FrozenTxsStorage + ChromaInfoStorage + Clone + Send + Sync + 'static,
{
    // The multiplication of average transaction size and max number of items
    // per request approximately gives the maximum JSON RPC request size.
    //
    // See `providelistyuvproofs`
    let max_request_body_size = AVERAGE_TX_SIZE * max_items_per_request;

    let server = Server::builder()
        .max_request_body_size(max_request_body_size as u32)
        .build(address)
        .await?;

    let handle = server.start(
        TransactionsController::new(
            txs_storage,
            full_event_bus,
            txs_states_storage,
            frozen_storage,
            bitcoin_client,
            max_items_per_request,
        )
        .into_rpc(),
    );

    // Await until stop message received
    cancellation.cancelled().await;

    // Send stop message to server
    if let Err(err) = handle.stop() {
        tracing::trace!("Failed to stop server: {}", err);
    }

    // Wait until server stopped
    handle.stopped().await;

    Ok(())
}
