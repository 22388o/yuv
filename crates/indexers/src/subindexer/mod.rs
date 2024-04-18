//! This module provides interface for sub-indexers, and some implementations.

use async_trait::async_trait;

mod freeze;
use bitcoin_client::json::GetBlockTxResult;
pub use freeze::FreezesIndexer;

mod confirmation;
pub use confirmation::ConfirmationIndexer;

/// Represents a sub-indexer, which is responsible for indexing a specific items
/// from a block.
#[async_trait]
pub trait Subindexer: Send + Sync + 'static {
    async fn index(&mut self, block: &GetBlockTxResult) -> eyre::Result<()>;
}
