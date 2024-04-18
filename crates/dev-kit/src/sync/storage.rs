use std::collections::HashMap;

use bitcoin::OutPoint;
use jsonrpsee::core::async_trait;
use yuv_pixels::PixelProof;
use yuv_storage::KeyValueStorage;

const UNSPENT_YUV_OUTPOINTS_KEY: &[u8] = b"unspent_yuv_txs";

#[derive(Default, Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UnspentYuvOutPointsEntry(HashMap<OutPoint, PixelProof>);

impl TryFrom<UnspentYuvOutPointsEntry> for Vec<u8> {
    type Error = serde_cbor::Error;

    fn try_from(value: UnspentYuvOutPointsEntry) -> Result<Self, Self::Error> {
        let bytes = serde_cbor::to_vec(&value.0)?;

        Ok(bytes)
    }
}

impl TryFrom<Vec<u8>> for UnspentYuvOutPointsEntry {
    type Error = serde_cbor::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let entry = serde_cbor::from_slice(&value)?;

        Ok(entry)
    }
}

#[async_trait]
pub trait UnspentYuvOutPointsStorage: KeyValueStorage<Vec<u8>, UnspentYuvOutPointsEntry> {
    async fn get_unspent_yuv_outpoints(&self) -> eyre::Result<HashMap<OutPoint, PixelProof>> {
        let entry = self
            .get(UNSPENT_YUV_OUTPOINTS_KEY.to_vec())
            .await?
            .unwrap_or_default();

        Ok(entry.0)
    }

    async fn put_unspent_yuv_outpoints(
        &self,
        unspent_yuv_outpoints: HashMap<OutPoint, PixelProof>,
    ) -> eyre::Result<()> {
        let entry = UnspentYuvOutPointsEntry(unspent_yuv_outpoints);

        self.put(UNSPENT_YUV_OUTPOINTS_KEY.to_vec(), entry).await?;

        Ok(())
    }
}

impl UnspentYuvOutPointsStorage for yuv_storage::LevelDB {}
