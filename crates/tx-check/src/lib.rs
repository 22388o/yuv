#![doc = include_str!("../README.md")]

mod errors;
pub use errors::CheckError;

mod transaction;
pub use transaction::check_transaction;

mod tx_checker;
pub use tx_checker::TransactionChecker;

mod worker;
pub use worker::{Config, TxCheckerWorker};

mod worker_pool;
pub use worker_pool::TxCheckerWorkerPool;

#[cfg(test)]
mod tests;
