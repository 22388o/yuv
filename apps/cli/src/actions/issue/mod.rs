use bdk::blockchain::Blockchain;
use clap::Args;
use color_eyre::eyre;
use yuv_pixels::Chroma;
use yuv_rpc_api::transactions::YuvTransactionsRpcClient;

use crate::context::Context;

pub const DEFAULT_SATOSHIS: u64 = 10_000;

#[derive(Args, Debug)]
pub struct IssueArgs {
    /// Amount in satoshis that will be added to YUV UTXO.
    #[clap(long, short, default_value_t = DEFAULT_SATOSHIS)]
    pub satoshis: u64,
    /// YUV token amount
    #[clap(long)]
    pub amount: u64,
    /// Public key of the recipient.
    #[clap(long)]
    #[arg(value_parser = Chroma::from_address)]
    pub recipient: Chroma,
    /// Provide proof of the transaction to YUV node.
    #[clap(long)]
    pub do_not_provide_proofs: bool,
}

pub async fn run(
    IssueArgs {
        amount,
        recipient,
        satoshis,
        do_not_provide_proofs,
    }: IssueArgs,
    mut ctx: Context,
) -> eyre::Result<()> {
    let wallet = ctx.wallet().await?;
    let blockchain = ctx.blockchain()?;
    let cfg = ctx.config()?;

    let tx = {
        let mut builder = wallet.build_issuance()?;

        builder
            .add_recipient(&recipient.public_key().inner, amount, satoshis)
            .set_fee_rate_strategy(cfg.fee_rate_strategy);

        builder.finish(&blockchain).await?
    };

    let txid = tx.bitcoin_tx.txid();
    let tx_type = tx.tx_type.clone();

    if do_not_provide_proofs {
        blockchain.broadcast(&tx.bitcoin_tx)?;
    } else {
        let yuv_client = ctx.yuv_client()?;

        yuv_client.send_raw_yuv_tx(tx).await?;
    }

    println!("tx id: {}", txid);
    println!("{}", serde_yaml::to_string(&tx_type)?);

    Ok(())
}
