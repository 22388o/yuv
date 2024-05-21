use bitcoin::Txid;
use yuv_storage::{ChromaInfoStorage, FrozenTxsStorage, InvalidTxsStorage, TransactionsStorage};
use yuv_types::announcements::{ChromaAnnouncement, FreezeAnnouncement, IssueAnnouncement};

use crate::TxCheckerWorker;

impl<TS, SS> TxCheckerWorker<TS, SS>
where
    TS: TransactionsStorage + Clone + Send + Sync + 'static,
    SS: InvalidTxsStorage + FrozenTxsStorage + ChromaInfoStorage + Clone + Send + Sync + 'static,
{
    /// Update chroma announcements in storage.
    pub(crate) async fn add_chroma_announcements(
        &self,
        announcement: &ChromaAnnouncement,
    ) -> eyre::Result<()> {
        let chroma_info = self
            .state_storage
            .get_chroma_info(&announcement.chroma)
            .await?;

        let total_supply = if let Some(chroma_info) = chroma_info {
            if chroma_info.announcement.is_some() {
                tracing::debug!(
                    "Chroma announcement for Chroma {} already exist",
                    announcement.chroma
                );

                return Ok(());
            }

            chroma_info.total_supply
        } else {
            0
        };

        self.state_storage
            .put_chroma_info(
                &announcement.chroma,
                Some(announcement.clone()),
                total_supply,
            )
            .await?;

        tracing::debug!(
            "Chroma announcement for Chroma {} is added",
            announcement.chroma
        );

        Ok(())
    }

    /// For each freeze toggle, update entry in freeze state storage.
    pub(crate) async fn update_freezes(
        &self,
        txid: Txid,
        freeze: &FreezeAnnouncement,
    ) -> eyre::Result<()> {
        let freeze_outpoint = &freeze.freeze_outpoint();

        let mut freeze_entry = self
            .state_storage
            .get_frozen_tx(freeze_outpoint)
            .await?
            .unwrap_or_default();

        freeze_entry.tx_ids.push(txid);

        tracing::debug!(
            "Freeze toggle for txid={} vout={} is set to {:?}",
            freeze.freeze_txid(),
            freeze_outpoint,
            freeze_entry.tx_ids,
        );

        self.state_storage
            .put_frozen_tx(freeze_outpoint, freeze_entry.tx_ids)
            .await?;

        Ok(())
    }

    pub(crate) async fn update_supply(&self, issue: &IssueAnnouncement) -> eyre::Result<()> {
        if let Some(chroma_info) = self.state_storage.get_chroma_info(&issue.chroma).await? {
            self.state_storage
                .put_chroma_info(
                    &issue.chroma,
                    chroma_info.announcement,
                    chroma_info.total_supply + issue.amount,
                )
                .await?;

            return Ok(());
        }

        self.state_storage
            .put_chroma_info(&issue.chroma, None, issue.amount)
            .await?;

        tracing::debug!("Updated supply for chroma {}", issue.chroma);

        Ok(())
    }
}
