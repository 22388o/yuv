use crate::FreezeTxToggle;
use bitcoin::Transaction;
use yuv_pixels::PixelProof;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

mod errors;
pub(crate) mod state;
pub(crate) mod toggle;

/// Represents entries of the YUV transaction inside the node's storage and
/// P2P communication inventory
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct YuvTransaction {
    pub bitcoin_tx: Transaction,
    pub tx_type: YuvTxType,
}

impl YuvTransaction {
    /// Create [`YuvTransaction`] with [`YuvTxType::FreezeToggle`] type for the
    /// given freezes and bitcoin transaction
    pub fn freeze_toggles(freezes: Vec<FreezeTxToggle>, bitcoin_tx: Transaction) -> Self {
        Self {
            bitcoin_tx,
            tx_type: YuvTxType::FreezeToggle { freezes },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", content = "data"))]
pub enum YuvTxType {
    Issue {
        output_proofs: BTreeMap<u32, PixelProof>,
    },
    Transfer {
        input_proofs: BTreeMap<u32, PixelProof>,
        output_proofs: BTreeMap<u32, PixelProof>,
    },
    FreezeToggle {
        freezes: Vec<FreezeTxToggle>,
    },
}

impl YuvTxType {
    /// Return output proofs if possible
    pub fn output_proofs(&self) -> Option<&BTreeMap<u32, PixelProof>> {
        match self {
            Self::Issue { output_proofs } => Some(output_proofs),
            Self::Transfer { output_proofs, .. } => Some(output_proofs),
            _ => None,
        }
    }

    /// Return input proofs if possible
    pub fn input_proofs(&self) -> Option<&BTreeMap<u32, PixelProof>> {
        match self {
            Self::Transfer { input_proofs, .. } => Some(input_proofs),
            _ => None,
        }
    }
}

impl Default for YuvTxType {
    fn default() -> Self {
        Self::Issue {
            output_proofs: Default::default(),
        }
    }
}

#[cfg(feature = "serde")]
impl TryFrom<YuvTransaction> for Vec<u8> {
    type Error = serde_cbor::Error;

    fn try_from(value: YuvTransaction) -> Result<Self, Self::Error> {
        serde_cbor::to_vec(&value)
    }
}

#[cfg(feature = "serde")]
impl TryFrom<Vec<u8>> for YuvTransaction {
    type Error = serde_cbor::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(&value)
    }
}
