//! This module provides a main indexer: [`BitcoinBlockIndexer`].

use bitcoin::BlockHash;
use bitcoin_client::{json::GetBlockTxResult, BitcoinRpcApi, BitcoinRpcClient};
use eyre::{bail, Context};
use futures::TryFutureExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use yuv_storage::BlockIndexerStorage;

use crate::{
    blockloader::{BlockLoaderConfig, IndexBlocksEvent},
    params::RunParams,
    BlockLoader, IndexingParams, Subindexer,
};

const BLOCK_CHUNK_SIZE: u64 = 1000;

/// Channel size between `Indexer` and `Blockloader`.  
const LOADED_BLOCKS_CHANNEL_SIZE: usize = 1;

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
    /// At start of the node, call this functions to index missed blocks and be up to date.
    #[instrument(skip_all)]
    pub async fn init(
        &mut self,
        params: IndexingParams,
        block_loader_config: BlockLoaderConfig,
        bitcoin_client: Arc<BitcoinRpcClient>,
        cancellation: CancellationToken,
    ) -> eyre::Result<()> {
        let starting_block_height = self.get_starting_block_height(&params).await?;

        tracing::info!(
            from_height = starting_block_height,
            "starting initial blocks indexing"
        );

        let block_loader = BlockLoader::new(
            bitcoin_client,
            block_loader_config.workers_number,
            block_loader_config.chunk_size,
        );

        let (sender_to_indexer, rx_indexer) = mpsc::channel(LOADED_BLOCKS_CHANNEL_SIZE);

        let handle = tokio::spawn(block_loader.run(
            starting_block_height,
            sender_to_indexer,
            block_loader_config.worker_time_sleep as u64,
            cancellation.child_token(),
        ))
        .map_err(|err| eyre::eyre!("Failed to run block loader: {}", err));

        let (blockloader_result, indexer_result) = tokio::join!(
            handle,
            self.handle_initial_blocks(rx_indexer, starting_block_height)
        );

        // 1 condition - Blockloader's join handle and just blockloader error weren't received but indexer's error was
        // 2 condition - Either blockloader's join handle error weren't received but blockloader
        // error and indexer errors were or blockloader's join handle and indexer errors were received
        // 3 conditon - Either received only blockloader join handle error or only blockloader error
        match (blockloader_result, indexer_result) {
            (Ok(Ok(_)), Err(indexer_error)) => return Err(indexer_error),
            (Ok(Err(blockloader_error)), Err(indexer_error))
            | (Err(blockloader_error), Err(indexer_error)) => {
                bail!(
                    "BlockLoader error: {}, Indexer error: {}",
                    blockloader_error,
                    indexer_error
                )
            }
            (Err(blockloader_error), Ok(_)) | (Ok(Err(blockloader_error)), Ok(_)) => {
                return Err(blockloader_error)
            }

            _ => {}
        }

        let last_indexed_hash = self
            .storage
            .get_last_indexed_hash()
            .await?
            .unwrap_or(self.bitcoin_client.get_block_hash(0).await?);

        tracing::info!(
            "initial blocks indexing finished at block with hash {}",
            last_indexed_hash
        );

        Ok(())
    }

    /// Return block height from indexing will start Returns 0 if [`IndexingParams
    /// ::yuv_genesis_block_hash`] is not provided and there is no `last_indexed_hash` in storage.
    /// Returns `last_indexed_height` if his block height is smaller than `yuv_genesis_block_height`
    /// and vice versa
    async fn get_starting_block_height(&self, params: &IndexingParams) -> eyre::Result<usize> {
        let mut starting_block_height = 0;

        let last_indexed_hash = self.storage.get_last_indexed_hash().await?;
        if let Some(last_indexed_hash) = last_indexed_hash {
            let last_indexed_height = self.get_block_height(&last_indexed_hash).await?;
            starting_block_height = last_indexed_height;
        }

        let params_hash = params.yuv_genesis_block_hash;
        if let Some(yuv_genesis_block_hash) = params_hash {
            let yuv_genesis_block_height = self.get_block_height(&yuv_genesis_block_hash).await?;

            if starting_block_height < yuv_genesis_block_height {
                starting_block_height = yuv_genesis_block_height;
            }
        }

        Ok(starting_block_height)
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
            }

            if let Err(err) = self.handle_new_blocks().await {
                tracing::error!("failed to run indexer: {:#}", err);
            }
        }
    }

    /// Index blocks from [`BlockLoader`]. It appears in `Indexer` init function. Handles blocks
    /// loading.
    ///
    /// # Errors
    ///
    /// Return an error, when cancellation event was received or if indexing of blocks failed
    async fn handle_initial_blocks(
        &mut self,
        mut rx_indexer: mpsc::Receiver<IndexBlocksEvent>,
        mut indexer_last_block_height: usize,
    ) -> eyre::Result<()> {
        while let Some(event) = rx_indexer.recv().await {
            match event {
                IndexBlocksEvent::FinishLoading => {
                    tracing::info!("finished loading the blocks");
                    break;
                }

                IndexBlocksEvent::LoadedBlocks(blocks) => {
                    self.init_blocks_handle(blocks, &mut indexer_last_block_height)
                        .await?;
                }
                IndexBlocksEvent::Cancelled => {
                    bail!("Cancelled node running, failed to index new blocks")
                }
            }
        }

        Ok(())
    }

    /// Initial blocks indexing. Receives blocks chunk from [`BlockLoader`] and indexes them.
    /// Returns an error, when blocks are not sequential.
    async fn init_blocks_handle(
        &mut self,
        blocks: Vec<GetBlockTxResult>,
        indexer_last_block_height: &mut usize,
    ) -> eyre::Result<()> {
        for block in blocks {
            if block.block_data.height.ne(indexer_last_block_height) {
                bail!(
                    "Blocks must be sequential, indexer_last_block_height: {} != block height: {}",
                    indexer_last_block_height,
                    block.block_data.height
                );
            }

            self.index_block(&block).await?;

            *indexer_last_block_height += 1;

            let height = block.block_data.height;
            tracing::trace!("indexed block at height {}", height);
            if height as u64 % BLOCK_CHUNK_SIZE == 0 {
                tracing::info!("indexed blocks at height: {}", height);
            }
        }

        Ok(())
    }

    /// Takes block, indexes it and puts its hash to storage as a `last_indexed_hash`.
    async fn index_block(&mut self, block: &GetBlockTxResult) -> eyre::Result<()> {
        for indexer in self.inner_indexers.iter_mut() {
            indexer
                .index(block)
                .await
                .wrap_err("failed to handle new block")?;
        }

        self.storage
            .put_last_indexed_hash(block.block_data.hash)
            .await?;

        Ok(())
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

        tracing::info!(
            best_block = ?current_block_hash,
            "New best block, indexing...",
        );

        // Go from last indexed block height to last block
        loop {
            let block = self
                .get_block_txs(current_block_hash)
                .await
                .wrap_err("failed to get block by hash")?;

            // TODO: consider using a `TaskTracker` here. Most likely, it will
            // require some changes in subindexers.
            self.index_block(&block).await?;

            let height = block.block_data.height;
            tracing::trace!("indexed block at height {}", height);

            let Some(next_block_hash) = block.block_data.nextblockhash else {
                break;
            };

            current_block_hash = next_block_hash;
        }

        Ok(())
    }

    async fn get_block_height(&self, hash: &BlockHash) -> eyre::Result<usize> {
        let block = self.bitcoin_client.get_block_info(hash).await?;
        Ok(block.block_data.height)
    }

    /// Get best block hash
    async fn get_best_block_hash(&self) -> eyre::Result<BlockHash> {
        self.bitcoin_client
            .get_best_block_hash()
            .await
            .wrap_err("failed to get best block hash")
    }

    /// Get block info by hash from Bitcoin RPC.
    async fn get_block_txs(&self, hash: BlockHash) -> eyre::Result<GetBlockTxResult> {
        self.bitcoin_client
            .get_block_txs(&hash)
            .await
            .wrap_err("failed to get block info by hash")
    }
}
