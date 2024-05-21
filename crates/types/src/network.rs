use core::{fmt::Display, str::FromStr};

use bitcoin::Network as BitcoinNetwork;

/// Mutiny network magic.
pub const MUTINY_MAGIC: u32 = 0xcb2ddfa5;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Default Bitcoin network types.
pub enum Network {
    Bitcoin,
    Testnet,
    Signet,
    Regtest,

    // Custom Bitcoin network types:
    Mutiny,
}

impl Network {
    pub fn to_bitcoin_network(&self) -> BitcoinNetwork {
        match self {
            Network::Bitcoin => BitcoinNetwork::Bitcoin,
            Network::Testnet => BitcoinNetwork::Testnet,
            Network::Signet => BitcoinNetwork::Signet,
            Network::Regtest => BitcoinNetwork::Regtest,
            _ => BitcoinNetwork::Testnet,
        }
    }

    pub fn magic(&self) -> u32 {
        // Mutiny network has custom network magic.
        if let Network::Mutiny = self {
            MUTINY_MAGIC
        } else {
            self.to_bitcoin_network().magic()
        }
    }
}

impl From<BitcoinNetwork> for Network {
    fn from(network: BitcoinNetwork) -> Self {
        match network {
            BitcoinNetwork::Bitcoin => Self::Bitcoin,
            BitcoinNetwork::Testnet => Self::Testnet,
            BitcoinNetwork::Signet => Self::Testnet,
            BitcoinNetwork::Regtest => Self::Regtest,
        }
    }
}

impl FromStr for Network {
    type Err = NetworkParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bitcoin" => Ok(Self::Bitcoin),
            "testnet" => Ok(Self::Testnet),
            "regtest" => Ok(Self::Regtest),
            "signet" => Ok(Self::Signet),
            "mutiny" => Ok(Self::Mutiny),
            _ => Err(NetworkParseError::UnknownType),
        }
    }
}

#[derive(Debug)]
pub enum NetworkParseError {
    UnknownType,
}

impl Display for NetworkParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            NetworkParseError::UnknownType => write!(f, "Unknown network type"),
        }
    }
}

#[cfg(not(feature = "no-std"))]
impl std::error::Error for NetworkParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NetworkParseError::UnknownType => None,
        }
    }
}
