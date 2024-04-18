#![doc = include_str!("../README.md")]
mod traits;
pub use traits::KeyValueError;
pub use traits::{
    BlockIndexerStorage, FrozenTxsStorage, InvalidTxsStorage, InventoryStorage, KeyValueResult,
    KeyValueStorage, PagesNumberStorage, PagesStorage, TransactionsStorage,
};

mod txstates;
pub use txstates::TxStatesStorage;

mod impls;
#[cfg(feature = "leveldb")]
pub use impls::leveldb::{
    FlushStrategy, LevelDB, Options as LevelDbOptions, DEFAULT_FLUSH_PERIOD_SECS,
};
