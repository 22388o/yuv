use bitcoin::Txid;
use clap::Args;
use color_eyre::eyre::{self, bail};

use yuv_pixels::{generate_bulletproof, Chroma};
use yuv_rpc_api::transactions::{GetRawYuvTransactionResponse, YuvTransactionsRpcClient};

use crate::context::Context;

use super::ecdh;

#[derive(Args, Debug)]
pub struct TransferArgs {
    /// Value to transfer
    #[clap(long)]
    pub value: u128,

    /// Value to transfer to sender
    #[clap(long)]
    pub residual: u128,

    #[clap(long)]
    /// Satoshis to transfer
    pub satoshis: u64,

    #[clap(long)]
    /// satoshis to transfer to sender
    pub residual_satoshis: u64,

    /// Type of the token, public key of the issuer.
    #[clap(long, value_parser = Chroma::from_address)]
    pub chroma: Chroma,

    /// The public key of the receiver.
    #[clap(long, value_parser = Chroma::from_address)]
    pub recipient: Chroma,

    /// The input tx id
    #[clap(long)]
    pub input_tx_id: Txid,

    /// The input tx vout
    #[clap(long)]
    pub input_tx_vout: u32,
}

pub async fn run(
    TransferArgs {
        value,
        residual,
        satoshis,
        residual_satoshis,
        chroma,
        recipient,
        input_tx_id,
        input_tx_vout,
    }: TransferArgs,
    mut context: Context,
) -> eyre::Result<()> {
    let config = context.config()?;
    let wallet = context.wallet().await?;
    let blockchain = context.blockchain()?;
    let yuv_client = context.yuv_client()?;

    let recipient = recipient.public_key();

    // Retrieve the bulletproof input tx
    let input_tx = yuv_client.get_raw_yuv_transaction(input_tx_id).await?;
    let GetRawYuvTransactionResponse::Attached(finished_input_tx) = input_tx else {
        bail!("The input tx is not finished")
    };

    let inputs = finished_input_tx
        .tx_type
        .output_proofs()
        .ok_or(eyre::eyre!("The input tx is not transfer or issuance"))?;

    let pixel_proof = inputs
        .get(&input_tx_vout)
        .ok_or(eyre::eyre!("The input tx does not contain the vout"))?;

    if !pixel_proof.is_bulletproof() {
        bail!("The input tx does not contain a bulletproof")
    }

    // Calculate the deffie hellman key
    let dh_key = ecdh(config.private_key, recipient, config.network())?;
    let dh_pub_key = dh_key.public_key(context.secp_ctx());
    let raw_dh_key: [u8; 32] = dh_key
        .to_bytes()
        .try_into()
        .expect("should convert to array");

    // Generate the bulletproofs with commitments
    let (value_proof, value_commit) = generate_bulletproof(value, raw_dh_key);
    let value_proof_hash = super::sha256(&value_proof.to_bytes())?;

    let (residual_proof, residual_commit) = generate_bulletproof(residual, raw_dh_key);
    let residual_proof_hash = super::sha256(&residual_proof.to_bytes())?;

    let mut builder = wallet.build_transfer()?;
    // Add the input tx
    builder.manual_selected_only();
    builder.add_bulletproof_input(input_tx_id, input_tx_vout);
    // Add the outputs
    builder.add_recipient_with_bulletproof(
        chroma,
        &recipient.inner,
        value_proof_hash,
        satoshis,
        value_commit,
        &dh_pub_key,
        value_proof,
    )?;
    builder
        .add_recipient_with_bulletproof(
            chroma,
            &config.private_key.public_key(context.secp_ctx()).inner,
            residual_proof_hash,
            residual_satoshis,
            residual_commit,
            &dh_pub_key,
            residual_proof,
        )?
        .set_fee_rate_strategy(config.fee_rate_strategy);

    let tx = builder.finish(&blockchain).await?;

    println!("{}", tx.bitcoin_tx.txid());

    yuv_client.send_raw_yuv_tx(tx, None).await?;

    Ok(())
}
