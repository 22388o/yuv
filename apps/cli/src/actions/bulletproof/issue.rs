use clap::Args;
use color_eyre::eyre;
use yuv_pixels::{generate_bulletproof, Chroma};
use yuv_rpc_api::transactions::YuvTransactionsRpcClient;

use super::ecdh;
use crate::context::Context;

const DEFAULT_SATOSHIS: u64 = 10_000;

#[derive(Args, Debug)]
pub struct IssueArgs {
    #[clap(long, short, default_value_t = DEFAULT_SATOSHIS)]
    pub satoshis: u64,

    /// Value to issue
    #[clap(long)]
    pub value: u128,

    /// Public key of the recipient.
    #[clap(long, value_parser = Chroma::from_address)]
    pub recipient: Chroma,
}

pub async fn run(
    IssueArgs {
        satoshis,
        value,
        recipient,
    }: IssueArgs,
    mut context: Context,
) -> eyre::Result<()> {
    let recipient = recipient.public_key();
    let config = context.config()?;
    let wallet = context.wallet().await?;
    let blockchain = context.blockchain()?;
    let yuv_client = context.yuv_client()?;

    let dh_key = ecdh(config.private_key, recipient, config.network())?;
    let dh_pub_key = dh_key.public_key(context.secp_ctx());

    let raw_dh_key: [u8; 32] = dh_key
        .to_bytes()
        .try_into()
        .expect("should convert to array");

    let (proof, commitment) = generate_bulletproof(value, raw_dh_key);

    let proof_hash = super::sha256(&proof.to_bytes())?;

    let mut builder = wallet.build_issuance()?;
    builder
        .add_recipient_with_bulletproof(
            &recipient.inner,
            proof_hash,
            satoshis,
            commitment,
            &dh_pub_key,
            proof,
        )?
        .set_fee_rate_strategy(config.fee_rate_strategy);

    let tx = builder.finish(&blockchain).await?;

    println!("{}", tx.bitcoin_tx.txid());

    yuv_client.send_raw_yuv_tx(tx, None).await?;

    Ok(())
}
