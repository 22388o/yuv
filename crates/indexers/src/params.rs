use std::time::Duration;

use bitcoin::BlockHash;

/// Parameters to specify for initial indexing of blocks,
/// that node have skipped.
pub struct IndexingParams {
    /// The hash of block from which indexing should start if
    /// there is no last indexed block hash in storage.
    pub yuv_genesis_block_hash: Option<BlockHash>,

    /// Number of blocks to index again (subtract from height
    /// of last indexed block).
    ///
    /// This is required, if node was shutdown ungracefully,
    /// and we don't know which blocks were indexed and which
    /// were not.
    pub index_step_back: u64,
}

impl Default for IndexingParams {
    fn default() -> Self {
        Self {
            yuv_genesis_block_hash: Default::default(),
            index_step_back: 1,
        }
    }
}

/// Parameters that are passed to the `run` method of the indexer.
#[derive(Debug)]
pub struct RunParams {
    /// Period of time to wait between polling new blocks from Bitcoin.
    pub polling_period: Duration,
}

impl Default for RunParams {
    fn default() -> Self {
        Self {
            polling_period: Duration::from_secs(10),
        }
    }
}
