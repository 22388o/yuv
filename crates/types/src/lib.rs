#![cfg_attr(feature = "no-std", no_std)]

#[cfg(not(any(feature = "std", feature = "no-std")))]
compile_error!("at least one of the `std` or `no-std` features must be enabled");

extern crate alloc;

mod transactions;

pub use transactions::{
    state::TxState,
    toggle::{FreezeTxToggle, TxFreezesEntry},
    YuvTransaction, YuvTxType,
};

#[cfg(all(feature = "messages", feature = "std"))]
pub mod messages;
#[cfg(all(feature = "messages", feature = "std"))]
pub use messages::{
    ConfirmationIndexerMessage, ControllerMessage, ControllerP2PMessage, GraphBuilderMessage,
    TxCheckerMessage,
};

#[cfg(feature = "consensus")]
mod consensus;

mod proofs;
pub use proofs::{ProofMap, Proofs};
