//! This module provides a main indexer: [`BitcoinBlockIndexer`].

use bitcoin::BlockHash;
use bitcoin_client::{
    json::GetBlockTxResult, BitcoinRpcApi, Error as BitcoinRpcError, JsonRpcError,
};
use eyre::Context;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use yuv_storage::BlockIndexerStorage;

use crate::{params::RunParams, IndexingParams, Subindexer};

const BLOCK_CHUNK_SIZE: u64 = 1000;

/// Using polling indexes blocks from Bitcoin and broadcasts it to inner indexers.
pub struct BitcoinBlockIndexer<BS, BC>
where
    BS: BlockIndexerStorage,
    BC: BitcoinRpcApi + Send + Sync + 'static,
{
    /// Bitcoin RPC Client
    bitcoin_client: Arc<BC>,

    /// Storage for block indexer
    storage: BS,

    /// Inner indexers for block indexer
    inner_indexers: Vec<Box<dyn Subindexer>>,
}

impl<BS, BC> BitcoinBlockIndexer<BS, BC>
where
    BS: BlockIndexerStorage + Send + Sync + 'static,
    BC: BitcoinRpcApi + Send + Sync + 'static,
{
    pub fn new(bitcoin_client: Arc<BC>, storage: BS) -> Self {
        Self {
            bitcoin_client,
            storage,
            inner_indexers: Vec::new(),
        }
    }

    pub fn add_indexer<I>(&mut self, indexer: I)
    where
        I: Subindexer + Send + Sync + 'static,
    {
        self.inner_indexers.push(Box::new(indexer));
    }

    /// Start indexing missed blocks from Bitcoin.
    ///
    /// At start of the node, call this functions to index
    /// missed blocks and be up to date.
    pub async fn init(&mut self, params: IndexingParams) -> eyre::Result<()> {
        let block_hash = if let Some(block_hash) = self.get_starting_block(&params).await? {
            block_hash
        } else {
            self.get_block_hash(0)
                .await?
                .expect("Genesis block should always exist")
        };

        self.storage.put_last_indexed_hash(block_hash).await?;

        tracing::info!("starting initial blocks indexing from block {}", block_hash);

        self.handle_new_blocks().await?;

        tracing::info!(
            "initial blocks indexing finished at block {}",
            self.storage
                .get_last_indexed_hash()
                .await?
                .expect("Last indexed hash should always be Some after initial indexing")
        );

        Ok(())
    }

    /// Return the block hash from which we should start initial indexing.
    ///
    /// If there is some in parameters, return it, if no, get one from storage.
    async fn get_starting_block(&self, params: &IndexingParams) -> eyre::Result<Option<BlockHash>> {
        if let Some(genesis) = params.yuv_genesis_block_hash {
            return Ok(Some(genesis));
        }

        self.storage
            .get_last_indexed_hash()
            .await
            .wrap_err("failed to get last indexed block hash")
    }

    /// Run indexer in loop, polling new blocks from Bitcoin RPC.
    pub async fn run(mut self, params: RunParams, cancellation: CancellationToken) {
        tracing::info!("starting bitcoin indexer, parameters: {:?}", params);

        let mut timer = tokio::time::interval(params.polling_period);

        loop {
            tokio::select! {
                _ = timer.tick() => {},
                _ = cancellation.cancelled() => {
                    tracing::trace!("cancellation received, stopping indexer");
                    return;
                }
            };

            if let Err(err) = self.handle_new_blocks().await {
                tracing::error!("failed to run indexer: {:#}", err);
            }
        }
    }

    /// Handle new block from Bitcoin RPC.
    async fn handle_new_blocks(&mut self) -> eyre::Result<()> {
        let best_block_hash = self.get_best_block_hash().await?;
        let mut current_block_hash = self
            .storage
            .get_last_indexed_hash()
            .await?
            .unwrap_or(best_block_hash);

        if current_block_hash.eq(&best_block_hash) {
            return Ok(());
        }

        // Go from last indexed block height to last block
        loop {
            let block = self
                .get_block_txs(current_block_hash)
                .await
                .wrap_err("failed to get block by hash")?;

            // TODO: consider using a `TaskTracker` here. Most likely, it will
            // require some changes in subindexers.
            for indexer in self.inner_indexers.iter_mut() {
                indexer
                    .index(&block)
                    .await
                    .wrap_err("failed to handle new block")?;
            }

            self.storage
                .put_last_indexed_hash(current_block_hash)
                .await?;

            let height = block.block_data.height;
            tracing::debug!("indexed block at height {}", height);
            if height as u64 % BLOCK_CHUNK_SIZE == 0 {
                tracing::info!("indexed blocks at height: {}", height);
            }

            let Some(next_block_hash) = block.block_data.nextblockhash else {
                break;
            };

            current_block_hash = next_block_hash;
        }

        Ok(())
    }

    /// Get best block hash
    async fn get_best_block_hash(&self) -> eyre::Result<BlockHash> {
        self.bitcoin_client
            .get_best_block_hash()
            .await
            .wrap_err("failed to get best block hash")
    }

    /// Get block hash by height from Bitcoin RPC.
    async fn get_block_hash(&self, height: u64) -> eyre::Result<Option<BlockHash>> {
        match self.bitcoin_client.get_block_hash(height).await {
            Ok(block_hash) => Ok(Some(block_hash)),
            Err(BitcoinRpcError::JsonRpc(JsonRpcError::Rpc(rpc_error)))
                if rpc_error.code == BLOCK_HASH_OUT_OF_RANGE =>
            {
                Ok(None)
            }
            Err(err) => Err(err).wrap_err("failed to get block hash"),
        }
    }

    /// Get block info by hash from Bitcoin RPC.
    async fn get_block_txs(&self, hash: BlockHash) -> eyre::Result<GetBlockTxResult> {
        self.bitcoin_client
            .get_block_txs(&hash)
            .await
            .wrap_err("failed to get block info by hash")
    }
}

/// Error code returned from Bitcoin RPC when block hash is out of range.
pub(crate) const BLOCK_HASH_OUT_OF_RANGE: i32 = -8;
