use std::mem::size_of;

use async_trait::async_trait;
use bitcoin::{OutPoint, Txid};
use serde_bytes::ByteArray;
use yuv_types::TxFreezesEntry;

use crate::{KeyValueResult, KeyValueStorage};

const FROZEN_PREFIX_SIZE: usize = 4;
const TXID_SIZE: usize = size_of::<Txid>();
const FROZEN_PREFIX: &[u8; FROZEN_PREFIX_SIZE] = b"frz-";
/// Frozen transactions storage key size is 4(`FROZEN_PREFIX:[u8; 4]`) + 32(`txid:Txid`) + 4(`vout:u32`) = 40 bytes long
const FROZEN_TX_STORAGE_KEY_SIZE: usize = FROZEN_PREFIX_SIZE + TXID_SIZE + size_of::<u32>();

fn frozen_tx_storage_key(outpoint: OutPoint) -> ByteArray<FROZEN_TX_STORAGE_KEY_SIZE> {
    let mut bytes = [0u8; FROZEN_TX_STORAGE_KEY_SIZE];

    bytes[..FROZEN_PREFIX_SIZE].copy_from_slice(FROZEN_PREFIX);
    bytes[FROZEN_PREFIX_SIZE..FROZEN_PREFIX_SIZE + TXID_SIZE].copy_from_slice(&outpoint.txid);
    bytes[FROZEN_PREFIX_SIZE + TXID_SIZE..].copy_from_slice(&outpoint.vout.to_be_bytes());

    ByteArray::new(bytes)
}

#[async_trait]
pub trait FrozenTxsStorage:
    KeyValueStorage<ByteArray<FROZEN_TX_STORAGE_KEY_SIZE>, TxFreezesEntry>
{
    async fn get_frozen_tx(&self, outpoint: OutPoint) -> KeyValueResult<Option<TxFreezesEntry>> {
        self.get(frozen_tx_storage_key(outpoint)).await
    }

    async fn put_frozen_tx(&self, outpoint: OutPoint, freeze_txs: Vec<Txid>) -> KeyValueResult<()> {
        self.put(
            frozen_tx_storage_key(outpoint),
            TxFreezesEntry::from(freeze_txs),
        )
        .await
    }

    async fn delete_frozen_tx(&self, outpoint: OutPoint) -> KeyValueResult<()> {
        self.delete(frozen_tx_storage_key(outpoint)).await
    }
}
