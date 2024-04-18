//! Sub-indexer for freeze toggles.

use async_trait::async_trait;
use bitcoin_client::json::GetBlockTxResult;
use event_bus::{typeid, EventBus};
use yuv_types::{ControllerMessage, FreezeTxToggle, YuvTransaction, YuvTxType};

use super::Subindexer;

/// A sub-indexer which gets freeze toggles from blocks and sends them to message handler.
pub struct FreezesIndexer {
    /// Event bus to message handler to notify about new freezes.
    event_bus: EventBus,
}

impl FreezesIndexer {
    pub fn new(full_event_bus: &EventBus) -> Self {
        let event_bus = full_event_bus
            .extract(&typeid![ControllerMessage], &[])
            .expect("message to message handler must be registered");

        Self { event_bus }
    }

    /// Finds freeze toggles in a block and sends them to message handler.
    async fn find_freezes(&self, block: &GetBlockTxResult) -> eyre::Result<()> {
        let mut txs = Vec::new();

        // For each transaction, try to find freeze toggles.
        for tx in &block.tx {
            if tx.is_coin_base() {
                continue;
            }

            let mut freezes = Vec::new();

            // In each transaction output:
            for output in tx.output.iter() {
                let script = &output.script_pubkey;

                // If it's not an OP_RETURN script, skip it.
                if !script.is_op_return() {
                    continue;
                }

                let freeze = match FreezeTxToggle::from_script(script) {
                    Ok(freeze) => freeze,
                    Err(err) => {
                        tracing::debug!("Invalid freeze toggle script: {}", err);
                        continue;
                    }
                };

                freezes.push(freeze);
            }

            if freezes.is_empty() {
                continue;
            }

            tracing::debug!("found {} freeze toggles at {}", freezes.len(), tx.txid());
            txs.push(YuvTransaction {
                bitcoin_tx: tx.clone(),
                tx_type: YuvTxType::FreezeToggle { freezes },
            })
        }

        if !txs.is_empty() {
            self.event_bus.send(ControllerMessage::NewYuxTxs(txs)).await;
        }

        Ok(())
    }
}

#[async_trait]
impl Subindexer for FreezesIndexer {
    async fn index(&mut self, block: &GetBlockTxResult) -> eyre::Result<()> {
        self.find_freezes(block).await
    }
}
