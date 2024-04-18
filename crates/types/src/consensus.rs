use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use bitcoin::consensus::encode::Error as EncodeError;
use bitcoin::consensus::{encode, Decodable, Encodable};
use bitcoin::Transaction;

use core2::io;

use yuv_pixels::PixelProof;

#[cfg(all(feature = "messages", feature = "std"))]
use crate::messages::p2p::Inventory;
use crate::{FreezeTxToggle, YuvTransaction, YuvTxType};

const ISSUE_CONSENSUS_FLAG: u8 = 0u8;
const TRANSFER_CONSENSUS_FLAG: u8 = 1u8;
const FREEZE_CONSENSUS_FLAG: u8 = 2u8;

#[cfg(all(feature = "messages", feature = "std"))]
const INVENTORY_YTX_FLAG: u8 = 0u8;

impl Encodable for YuvTxType {
    fn consensus_encode<W: io::Write + ?Sized>(&self, writer: &mut W) -> Result<usize, io::Error> {
        let mut len = 0;

        match self {
            YuvTxType::Issue { output_proofs } => {
                len += ISSUE_CONSENSUS_FLAG.consensus_encode(writer)?;
                len += ProofsWrapper(output_proofs.clone()).consensus_encode(writer)?;
            }
            YuvTxType::Transfer {
                input_proofs,
                output_proofs,
            } => {
                len += TRANSFER_CONSENSUS_FLAG.consensus_encode(writer)?;
                len += ProofsWrapper(input_proofs.clone()).consensus_encode(writer)?;
                len += ProofsWrapper(output_proofs.clone()).consensus_encode(writer)?;
            }
            YuvTxType::FreezeToggle { freezes } => {
                len += FREEZE_CONSENSUS_FLAG.consensus_encode(writer)?;

                len += (freezes.len() as u32).consensus_encode(writer)?;

                for freeze in freezes {
                    len += freeze.consensus_encode(writer)?;
                }
            }
        }

        Ok(len)
    }
}

impl Decodable for YuvTxType {
    fn consensus_decode<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, EncodeError> {
        let kind: u8 = Decodable::consensus_decode(reader)?;

        match kind {
            ISSUE_CONSENSUS_FLAG => {
                let ProofsWrapper(output_proofs) = Decodable::consensus_decode(reader)?;

                Ok(YuvTxType::Issue { output_proofs })
            }
            TRANSFER_CONSENSUS_FLAG => {
                let ProofsWrapper(input_proofs) = Decodable::consensus_decode(reader)?;
                let ProofsWrapper(output_proofs) = Decodable::consensus_decode(reader)?;

                Ok(YuvTxType::Transfer {
                    input_proofs,
                    output_proofs,
                })
            }
            FREEZE_CONSENSUS_FLAG => {
                let len: u32 = Decodable::consensus_decode(reader)?;
                let freezes: Vec<FreezeTxToggle> = (0..len)
                    .map(|_i| Decodable::consensus_decode(reader))
                    .collect::<Result<Vec<_>, EncodeError>>()?;

                Ok(YuvTxType::FreezeToggle { freezes })
            }
            _ => Err(EncodeError::ParseFailed("Unknown YUV tx type")),
        }
    }
}

struct ProofsWrapper(BTreeMap<u32, PixelProof>);

impl Encodable for ProofsWrapper {
    fn consensus_encode<W: io::Write + ?Sized>(&self, writer: &mut W) -> Result<usize, io::Error> {
        let mut len = 0;

        len += (self.0.len() as u32).consensus_encode(writer)?;
        for (key, value) in self.0.iter() {
            len += key.consensus_encode(writer)?;
            len += value.consensus_encode(writer)?;
        }

        Ok(len)
    }
}

impl Decodable for ProofsWrapper {
    fn consensus_decode<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, encode::Error> {
        let len: u32 = Decodable::consensus_decode(reader)?;

        let mut proofs: BTreeMap<u32, PixelProof> = BTreeMap::new();

        for _ in 0..len {
            let key: u32 = Decodable::consensus_decode(reader)?;
            let value: PixelProof = Decodable::consensus_decode(reader)?;

            proofs.insert(key, value);
        }

        Ok(ProofsWrapper(proofs))
    }
}

impl Encodable for YuvTransaction {
    fn consensus_encode<W: io::Write + ?Sized>(&self, writer: &mut W) -> Result<usize, io::Error> {
        let mut len = 0;

        len += self.bitcoin_tx.consensus_encode(writer)?;
        len += self.tx_type.consensus_encode(writer)?;

        Ok(len)
    }
}

impl Decodable for YuvTransaction {
    fn consensus_decode<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, EncodeError> {
        let bitcoin_tx: Transaction = Decodable::consensus_decode(reader)?;
        let tx_type: YuvTxType = Decodable::consensus_decode(reader)?;

        Ok(YuvTransaction {
            bitcoin_tx,
            tx_type,
        })
    }
}

pub(crate) struct YuvTxsWrapper(pub Vec<YuvTransaction>);

impl Encodable for YuvTxsWrapper {
    fn consensus_encode<W: io::Write + ?Sized>(&self, writer: &mut W) -> Result<usize, io::Error> {
        let mut len = 0;

        len += (self.0.len() as u32).consensus_encode(writer)?;

        for tx in &self.0 {
            len += tx.consensus_encode(writer)?;
        }

        Ok(len)
    }
}

impl Decodable for YuvTxsWrapper {
    fn consensus_decode<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, EncodeError> {
        let len: u32 = Decodable::consensus_decode(reader)?;

        let txs: Vec<YuvTransaction> = (0..len)
            .map(|_i| Decodable::consensus_decode(reader))
            .collect::<Result<Vec<_>, EncodeError>>()?;

        Ok(YuvTxsWrapper(txs))
    }
}

#[cfg(all(feature = "messages", feature = "std"))]
impl Encodable for Inventory {
    fn consensus_encode<W: io::Write + ?Sized>(&self, writer: &mut W) -> Result<usize, io::Error> {
        let mut len = 0;

        match self {
            Inventory::Ytx(txid) => {
                len += INVENTORY_YTX_FLAG.consensus_encode(writer)?;
                len += txid.consensus_encode(writer)?;
            }
        }

        Ok(len)
    }
}

#[cfg(all(feature = "messages", feature = "std"))]
impl Decodable for Inventory {
    fn consensus_decode<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, EncodeError> {
        let flag: u8 = Decodable::consensus_decode(reader)?;

        match flag {
            INVENTORY_YTX_FLAG => Ok(Inventory::Ytx(Decodable::consensus_decode(reader)?)),
            _ => Err(EncodeError::ParseFailed("Unknown inventory type")),
        }
    }
}

#[cfg(all(feature = "messages", feature = "std"))]
pub(crate) struct InventoryWrapper(pub Vec<Inventory>);

#[cfg(all(feature = "messages", feature = "std"))]
impl Encodable for InventoryWrapper {
    fn consensus_encode<W: io::Write + ?Sized>(&self, writer: &mut W) -> Result<usize, io::Error> {
        let mut len = 0;

        len += (self.0.len() as u32).consensus_encode(writer)?;

        for inv in &self.0 {
            len += inv.consensus_encode(writer)?;
        }

        Ok(len)
    }
}

#[cfg(all(feature = "messages", feature = "std"))]
impl Decodable for InventoryWrapper {
    fn consensus_decode<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, EncodeError> {
        let len: u32 = Decodable::consensus_decode(reader)?;

        let inv: Vec<Inventory> = (0..len)
            .map(|_i| Decodable::consensus_decode(reader))
            .collect::<Result<Vec<_>, EncodeError>>()?;

        Ok(InventoryWrapper(inv))
    }
}

#[cfg(all(test, feature = "serde", feature = "messages", feature = "std"))]
mod tests {
    extern crate serde_json;

    use alloc::vec;
    use alloc::vec::Vec;

    use bitcoin::consensus::{Decodable, Encodable};
    use once_cell::sync::Lazy;

    use crate::{messages::p2p::Inventory, YuvTransaction};

    static YUV_TXS: Lazy<Vec<YuvTransaction>> = Lazy::new(|| {
        vec![
            serde_json::from_str::<YuvTransaction>(include_str!("./assets/transfer.json"))
                .expect("JSON was not well-formatted"),
            serde_json::from_str::<YuvTransaction>(include_str!("./assets/issue.json"))
                .expect("JSON was not well-formatted"),
        ]
    });

    #[test]
    fn test_yuv_tx_consensus_encode() {
        for tx in &*YUV_TXS {
            let mut bytes: Vec<u8> = Vec::new();
            tx.consensus_encode(&mut bytes)
                .expect("failed to encode the tx");

            let decoded_tx = YuvTransaction::consensus_decode(&mut bytes.as_slice())
                .expect("failed to decode the tx");

            assert_eq!(tx, &decoded_tx, "Converting back and forth should work")
        }
    }

    #[test]
    fn test_inventory_consensus_encode() {
        for tx in &*YUV_TXS {
            let inventory = Inventory::Ytx(tx.bitcoin_tx.txid());
            let mut bytes: Vec<u8> = Vec::new();
            inventory
                .consensus_encode(&mut bytes)
                .expect("failed to encode the inventory");

            let decoded_inventory = Inventory::consensus_decode(&mut bytes.as_slice())
                .expect("failed to decode the inventory");

            assert_eq!(
                inventory, decoded_inventory,
                "Converting back and forth should work"
            )
        }
    }
}
