use bitcoin::Txid;
use bitcoin_client::{BitcoinRpcApi, BitcoinRpcAuth, BitcoinRpcClient, Error as BitcoinRpcError};
use eyre::Result;
use yuv_types::{ProofMap, YuvTransaction, YuvTxType};

use crate::{errors::TxCheckerError, transaction::check_transaction};

/// Contains several steps to check that YUV Transactions is correct in terms of the YUV Protocol.
pub struct TransactionChecker {
    bitcoin_client: BitcoinRpcClient,
}

impl TransactionChecker {
    pub async fn from_auth(url: &str, auth: BitcoinRpcAuth) -> Result<Self, BitcoinRpcError> {
        Ok(Self {
            bitcoin_client: BitcoinRpcClient::new(auth, url.to_string()).await?,
        })
    }

    pub async fn check_p2wpkh_tx_by_id(
        &self,
        tx_id: &Txid,
        inputs: &ProofMap,
        outputs: &ProofMap,
    ) -> Result<(), TxCheckerError> {
        let yuv_tx_type = match inputs.is_empty() {
            true => YuvTxType::Issue {
                output_proofs: outputs.clone(),
            },
            false => YuvTxType::Transfer {
                input_proofs: inputs.clone(),
                output_proofs: outputs.clone(),
            },
        };

        // Check that transaction exists at all
        let tx = self.bitcoin_client.get_raw_transaction(tx_id, None).await?;

        check_transaction(&YuvTransaction {
            bitcoin_tx: tx,
            tx_type: yuv_tx_type,
        })?;

        Ok(())
    }
}
