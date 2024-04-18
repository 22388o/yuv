use alloc::vec::Vec;
use core::mem::size_of;

extern crate std;

use crate::transactions::errors::FreezeTxToggleParseError;
#[cfg(feature = "consensus")]
use bitcoin::consensus::{encode::Error, Decodable, Encodable};
use bitcoin::{
    blockdata::{
        opcodes::{
            all::{OP_PUSHBYTES_32, OP_RETURN},
            All as Opcodes,
        },
        script::{Builder, Instruction},
    },
    hashes::Hash,
    OutPoint, Script, Txid,
};

/// Size of txid in bytes.
const TX_ID_SIZE: usize = size_of::<Txid>();
/// Size of freeze entry in bytes.
pub const FREEZE_ENTRY_SIZE: usize = TX_ID_SIZE + size_of::<u32>();
/// Number of instructions in freeze script.
pub const FREEZE_INSTRUCTION_NUMBER: usize = 3;

/// An entry that appears in `OP_RETURN` when issuer declares
/// that tx is frozen or unfrozen.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreezeTxToggle {
    /// Transaction ID we are freezing.
    pub txid: Txid,
    /// Output of that transaction.
    pub vout: u32,
}

#[cfg(feature = "consensus")]
impl Encodable for FreezeTxToggle {
    fn consensus_encode<W: std::io::Write + ?Sized>(
        &self,
        writer: &mut W,
    ) -> Result<usize, std::io::Error> {
        let mut len = 0;

        len += self.txid.consensus_encode(writer)?;
        len += self.vout.consensus_encode(writer)?;

        Ok(len)
    }
}

#[cfg(feature = "consensus")]
impl Decodable for FreezeTxToggle {
    fn consensus_decode<R: std::io::Read + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let txid: Txid = Decodable::consensus_decode(reader)?;
        let vout: u32 = Decodable::consensus_decode(reader)?;

        Ok(FreezeTxToggle::new(txid, vout))
    }
}

impl FreezeTxToggle {
    pub fn new(txid: Txid, vout: u32) -> Self {
        Self { txid, vout }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(FREEZE_ENTRY_SIZE);

        bytes.extend_from_slice(&self.txid[..]);
        bytes.extend_from_slice(&self.vout.to_be_bytes());

        bytes
    }

    pub fn to_array(&self) -> [u8; FREEZE_ENTRY_SIZE] {
        let mut bytes = [0u8; FREEZE_ENTRY_SIZE];

        bytes[..].copy_from_slice(&self.to_bytes()[..]);

        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FreezeTxToggleParseError> {
        if bytes.len() != FREEZE_ENTRY_SIZE {
            return Err(FreezeTxToggleParseError::InvalidPushBytesSize(bytes.len()));
        }

        let mut txid_bytes = [0u8; TX_ID_SIZE];
        txid_bytes.copy_from_slice(&bytes[..TX_ID_SIZE]);

        let vout = u32::from_be_bytes(bytes[TX_ID_SIZE..].try_into().expect("Size is checked"));

        Ok(Self {
            txid: Txid::from_slice(&txid_bytes)?,
            vout,
        })
    }

    /// Convert Bitcoin script to freeze transaction.
    pub fn from_script(script: &Script) -> Result<Self, FreezeTxToggleParseError> {
        if !script.is_op_return() {
            return Err(FreezeTxToggleParseError::NoOpReturn);
        }

        let instructions = script.instructions().collect::<Result<Vec<_>, _>>()?;

        // OP_PUSHBYTES_32 in instruction is not stored, for some reason
        if instructions.len() != FREEZE_INSTRUCTION_NUMBER - 1 {
            return Err(FreezeTxToggleParseError::InvalidInstructionsNumber(
                instructions.len(),
            ));
        }

        match &instructions[1] {
            Instruction::PushBytes(bytes) => Self::from_bytes(bytes),
            inst => Err(FreezeTxToggleParseError::InvalidInstruction(
                instuction_into_opcode(inst),
            )),
        }
    }

    pub fn to_script(&self) -> Script {
        let slice = self.to_bytes();

        Builder::new()
            .push_opcode(OP_RETURN)
            .push_slice(&slice)
            .into_script()
    }
}

impl From<OutPoint> for FreezeTxToggle {
    fn from(outpoint: OutPoint) -> Self {
        Self {
            txid: outpoint.txid,
            vout: outpoint.vout,
        }
    }
}

impl From<FreezeTxToggle> for OutPoint {
    fn from(freeze_tx_toggle: FreezeTxToggle) -> Self {
        OutPoint {
            txid: freeze_tx_toggle.txid,
            vout: freeze_tx_toggle.vout,
        }
    }
}

fn instuction_into_opcode(inst: &Instruction) -> Opcodes {
    match inst {
        Instruction::Op(op) => *op,
        Instruction::PushBytes(_) => OP_PUSHBYTES_32,
    }
}

/// Entry that appears in `OP_RETURN` when issuer declares that tx is frozen or
/// unfronzen.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TxFreezesEntry {
    /// Identifiers of transaction that tried to freeze the output.
    pub tx_ids: Vec<Txid>,
}

impl From<Vec<Txid>> for TxFreezesEntry {
    fn from(value: Vec<Txid>) -> Self {
        Self { tx_ids: value }
    }
}

#[cfg(feature = "serde")]
impl TryFrom<TxFreezesEntry> for Vec<u8> {
    type Error = serde_cbor::Error;

    fn try_from(value: TxFreezesEntry) -> Result<Self, Self::Error> {
        serde_cbor::to_vec(&value)
    }
}

#[cfg(feature = "serde")]
impl TryFrom<Vec<u8>> for TxFreezesEntry {
    type Error = serde_cbor::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_cbor::from_slice(&value)
    }
}

impl TxFreezesEntry {
    /// Check if UTXO is frozen or not based on the number of txs that tried to
    /// freeze it.
    pub fn is_frozen(&self) -> bool {
        self.tx_ids.len() % 2 == 1
    }
}

#[cfg(test)]
mod tests {
    use crate::FreezeTxToggle;
    use bitcoin::Txid;
    use core::str::FromStr;
    use once_cell::sync::Lazy;

    static TXID: Lazy<Txid> = Lazy::new(|| {
        Txid::from_str("0000000000000000000000000000000000000000000000000000000000000000").unwrap()
    });

    #[test]
    fn freeze_tx_toggle_conv_test() {
        let freeze_toggle = FreezeTxToggle::new(*TXID, 0);
        let freeze_toggle_converted =
            FreezeTxToggle::from_bytes(freeze_toggle.to_bytes().as_slice())
                .expect("must be converted successfully as we passed bytes of a FreezeTxToggle");

        assert_eq!(freeze_toggle, freeze_toggle_converted)
    }

    /// Passed freeze toggles with different outputs
    #[test]
    fn freeze_tx_toggle_conv_invalid_test() {
        let freeze_toggle1 = FreezeTxToggle::new(*TXID, 0);
        let freeze_toggle1_converted =
            FreezeTxToggle::from_bytes(freeze_toggle1.to_bytes().as_slice())
                .expect("must be converted successfully as we passed bytes of a FreezeTxToggle");

        let freeze_toggle2 = FreezeTxToggle::new(*TXID, 1);
        let freeze_toggle2_converted =
            FreezeTxToggle::from_bytes(freeze_toggle2.to_bytes().as_slice())
                .expect("must be converted successfully as we passed bytes of a FreezeTxToggle");

        assert_ne!(freeze_toggle1_converted, freeze_toggle2_converted)
    }
}
