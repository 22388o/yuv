#![cfg_attr(feature = "no-std", no_std)]

extern crate alloc;
extern crate core;

pub use announcements::{Announcement, AnyAnnouncement};
#[cfg(all(feature = "messages", feature = "std"))]
pub use messages::{
    ConfirmationIndexerMessage, ControllerMessage, ControllerP2PMessage, GraphBuilderMessage,
    TxCheckerMessage,
};
pub use proofs::{ProofMap, TransferProofs};
pub use transactions::{YuvTransaction, YuvTxType};

#[cfg(not(any(feature = "std", feature = "no-std")))]
compile_error!("at least one of the `std` or `no-std` features must be enabled");

pub mod announcements;
mod transactions;

#[cfg(feature = "consensus")]
mod consensus;
#[cfg(all(feature = "messages", feature = "std"))]
pub mod messages;

mod proofs;
