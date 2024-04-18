use async_trait::async_trait;

mod transactions;
use serde::{de::DeserializeOwned, Serialize};
pub use transactions::TransactionsStorage;

mod invalid;
pub use invalid::InvalidTxsStorage;

mod inventory;
pub use inventory::InventoryStorage;

pub(crate) mod pages;
pub use pages::PagesNumberStorage;
pub use pages::PagesStorage;

mod indexed_block;
pub use indexed_block::BlockIndexerStorage;

mod frozen;
pub use frozen::FrozenTxsStorage;

#[derive(Debug, thiserror::Error)]
pub enum KeyValueError {
    #[error("Decoding error: {0}")]
    Decoding(serde_cbor::Error),
    #[error("Encoding error: {0}")]
    Encoding(serde_cbor::Error),
    #[error("Storage error: {0}")]
    Storage(Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub type KeyValueResult<T> = Result<T, KeyValueError>;

#[async_trait]
pub trait KeyValueStorage<K, V>
where
    K: Serialize + Send + Sync + 'static,
    V: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Error: std::error::Error + 'static + Send + Sync;

    async fn raw_put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), Self::Error>;
    async fn raw_get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, Self::Error>;
    async fn raw_delete(&self, key: Vec<u8>) -> Result<(), Self::Error>;

    async fn flush(&self) -> Result<(), Self::Error>;

    async fn put(&self, key: K, value: V) -> KeyValueResult<()> {
        let key: Vec<u8> = serde_cbor::to_vec(&key).map_err(KeyValueError::Encoding)?;
        let value: Vec<u8> = serde_cbor::to_vec(&value).map_err(KeyValueError::Encoding)?;

        self.raw_put(key, value)
            .await
            .map_err(|err| KeyValueError::Storage(Box::new(err)))
    }

    async fn get(&self, key: K) -> KeyValueResult<Option<V>> {
        let key: Vec<u8> = serde_cbor::to_vec(&key).map_err(KeyValueError::Encoding)?;

        let result = self
            .raw_get(key)
            .await
            .map_err(|err| KeyValueError::Storage(Box::new(err)))?;

        let Some(value) = result else {
            return Ok(None);
        };

        let value: V = serde_cbor::from_slice(&value).map_err(KeyValueError::Decoding)?;

        Ok(Some(value))
    }

    async fn delete(&self, key: K) -> KeyValueResult<()> {
        let key: Vec<u8> = serde_cbor::to_vec(&key).map_err(KeyValueError::Encoding)?;

        self.raw_delete(key)
            .await
            .map_err(|err| KeyValueError::Storage(Box::new(err)))
    }
}
