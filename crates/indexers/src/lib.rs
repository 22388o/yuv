//! This module provides a [`BitcoinBlockIndexer`] which indexes blocks from Bitcoin.
#![doc = include_str!("../README.md")]
mod params;
pub use params::{IndexingParams, RunParams};

#[cfg(test)]
mod tests;

mod indexer;
pub use indexer::BitcoinBlockIndexer;

mod subindexer;
pub use subindexer::{ConfirmationIndexer, FreezesIndexer, Subindexer};
