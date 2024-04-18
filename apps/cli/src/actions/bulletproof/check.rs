use bitcoin::Txid;
use clap::Args;
use color_eyre::eyre::{self, bail};
use yuv_pixels::{generate_bulletproof, Chroma};
use yuv_rpc_api::transactions::{GetRawYuvTransactionResponse, YuvTransactionsRpcClient};

use super::ecdh;
use crate::context::Context;

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Value to check
    #[clap(long)]
    pub value: u64,

    #[clap(long)]
    pub tx_id: Txid,

    #[clap(long)]
    pub tx_vout: u32,

    /// Sender public key
    #[clap(long)]
    #[arg(value_parser = Chroma::from_address)]
    pub sender: Chroma,
}

pub async fn run(
    CheckArgs {
        value,
        tx_id,
        tx_vout,
        sender,
    }: CheckArgs,
    mut context: Context,
) -> eyre::Result<()> {
    let config = context.config()?;
    let yuv_client = context.yuv_client()?;

    let dh_key = ecdh(config.private_key, sender.public_key(), config.network())?;

    let raw_dh_key: [u8; 32] = dh_key
        .to_bytes()
        .try_into()
        .expect("should convert to array");
    let (_, commit) = generate_bulletproof(value, raw_dh_key);

    let tx = yuv_client.get_raw_yuv_transaction(tx_id).await?;

    let GetRawYuvTransactionResponse::Attached(attached_tx) = tx else {
        bail!("The tx is not attached")
    };

    let output_proofs = attached_tx
        .tx_type
        .output_proofs()
        .ok_or_else(|| eyre::eyre!("The tx is not valid"))?;

    let proof = output_proofs
        .get(&tx_vout)
        .ok_or_else(|| eyre::eyre!("The tx vout is not valid"))?;

    let bulletproof = proof
        .get_bulletproof()
        .ok_or_else(|| eyre::eyre!("The tx pixel proof is not bulletproof"))?;

    if commit != bulletproof.commitment {
        return Err(eyre::eyre!("Invalid commitment"));
    }

    println!("Commit valid!");

    Ok(())
}
