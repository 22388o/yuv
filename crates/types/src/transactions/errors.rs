use bitcoin::blockdata::opcodes::All as Opcodes;
use core::fmt::{self, Display};

use crate::transactions::toggle::{FREEZE_ENTRY_SIZE, FREEZE_INSTRUCTION_NUMBER};

#[derive(Debug)]
pub enum FreezeTxToggleParseError {
    InvalidPushBytesSize(usize),
    InvalidInstructionsNumber(usize),
    InvalidTxHash(bitcoin::hashes::Error),
    NoOpReturn,
    InvalidInstruction(Opcodes),
    ScriptError(bitcoin::blockdata::script::Error),
}

impl Display for FreezeTxToggleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FreezeTxToggleParseError::InvalidPushBytesSize(size) => write!(
                f,
                "Invalid OP_PUSHBYTES size should be {}, got {}",
                FREEZE_ENTRY_SIZE, size
            ),
            FreezeTxToggleParseError::InvalidInstructionsNumber(num) => write!(
                f,
                "Invalid number of instructions, should be {}, got {}",
                FREEZE_INSTRUCTION_NUMBER, num
            ),
            FreezeTxToggleParseError::InvalidTxHash(e) => write!(f, "Invalid tx hash: {}", e),
            FreezeTxToggleParseError::NoOpReturn => write!(f, "No OP_RETURN in script"),
            FreezeTxToggleParseError::InvalidInstruction(opcode) => {
                write!(f, "Invalid opcode {}", opcode)
            }
            FreezeTxToggleParseError::ScriptError(e) => write!(f, "Script error: {}", e),
        }
    }
}

#[cfg(not(feature = "no-std"))]
impl std::error::Error for FreezeTxToggleParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FreezeTxToggleParseError::InvalidTxHash(e) => Some(e),
            FreezeTxToggleParseError::ScriptError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<bitcoin::hashes::Error> for FreezeTxToggleParseError {
    fn from(err: bitcoin::hashes::Error) -> Self {
        FreezeTxToggleParseError::InvalidTxHash(err)
    }
}

impl From<bitcoin::blockdata::script::Error> for FreezeTxToggleParseError {
    fn from(err: bitcoin::blockdata::script::Error) -> Self {
        FreezeTxToggleParseError::ScriptError(err)
    }
}
