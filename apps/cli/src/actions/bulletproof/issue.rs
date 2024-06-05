use clap::Args;
use color_eyre::eyre;
use ydk::txbuilder::BulletproofRecipientParameters;
use yuv_pixels::Chroma;
use yuv_rpc_api::transactions::YuvTransactionsRpcClient;

use crate::context::Context;

const DEFAULT_SATOSHIS: u64 = 10_000;

#[derive(Args, Debug)]
pub struct IssueArgs {
    #[clap(long, short, default_value_t = DEFAULT_SATOSHIS)]
    pub satoshis: u64,

    /// Amount to issue
    #[clap(long)]
    pub amount: u128,

    /// Public key of the recipient.
    #[clap(long, value_parser = Chroma::from_address)]
    pub recipient: Chroma,
}

pub async fn run(
    IssueArgs {
        satoshis,
        amount,
        recipient,
    }: IssueArgs,
    mut context: Context,
) -> eyre::Result<()> {
    let recipient = recipient.public_key();
    let config = context.config()?;
    let wallet = context.wallet().await?;
    let blockchain = context.blockchain()?;
    let yuv_client = context.yuv_client()?;

    let mut builder = wallet.build_issuance()?;
    builder
        .add_recipient_with_bulletproof(BulletproofRecipientParameters {
            recipient: recipient.inner,
            satoshis,
            amount,
        })?
        .set_fee_rate_strategy(config.fee_rate_strategy);

    let tx = builder.finish(&blockchain).await?;

    println!("{}", tx.bitcoin_tx.txid());

    yuv_client.send_raw_yuv_tx(tx, None).await?;

    Ok(())
}
