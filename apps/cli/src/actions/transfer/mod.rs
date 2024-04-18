use std::usize;

use crate::context::Context;
use bdk::blockchain::Blockchain;
use clap::Args;
use color_eyre::eyre::{self, bail, Ok};
use yuv_pixels::Chroma;
use yuv_rpc_api::transactions::YuvTransactionsRpcClient;

const DEFAULT_SATOSHIS: u64 = 1000;

#[derive(Args, Debug)]
pub struct TransferArgs {
    /// Amount to send.
    #[clap(long, short, num_args = 1..)]
    pub amount: Vec<u64>,

    /// Satoshis to spend. Specify it either once to override the default,
    /// or per chroma to use a different number of satoshis in each output.
    #[clap(long, short, num_args = 1.., default_values_t = vec![DEFAULT_SATOSHIS])]
    pub satoshis: Vec<u64>,

    /// Type of the token, public key of the issuer.
    #[clap(long, short, num_args = 1..)]
    #[arg(value_parser = Chroma::from_address)]
    pub chroma: Vec<Chroma>,

    /// The public key of the receiver.
    #[clap(long, short, num_args = 1..)]
    #[arg(value_parser = Chroma::from_address)]
    pub recipient: Vec<Chroma>,

    /// Provide proof of the transaction to YUV node or not.
    #[clap(long)]
    pub do_not_provide_proofs: bool,
}

// TODO: refactor this
pub async fn run(
    TransferArgs {
        amount,
        satoshis,
        chroma,
        recipient,
        do_not_provide_proofs,
    }: TransferArgs,
    mut ctx: Context,
) -> eyre::Result<()> {
    if chroma.len() != amount.len() || chroma.len() != recipient.len() {
        bail!("Number of chromas, amounts and recipients must correspond to each other")
    }
    let wallet = ctx.wallet().await?;
    let satoshis = process_satoshis(satoshis, chroma.len())?;
    let blockchain = ctx.blockchain()?;
    let cfg = ctx.config()?;

    let tx = {
        let mut builder = wallet.build_transfer()?;

        for i in 0..chroma.len() {
            builder.add_recipient(
                chroma[i],
                &recipient[i].public_key().inner,
                amount[i],
                satoshis[i],
            );
        }

        builder.set_fee_rate_strategy(cfg.fee_rate_strategy);

        builder.finish(&blockchain).await?
    };

    if do_not_provide_proofs {
        blockchain.broadcast(&tx.bitcoin_tx)?;
    } else {
        let client = ctx.yuv_client()?;

        client.send_raw_yuv_tx(tx.clone()).await?;
    }

    println!("tx id: {}", tx.bitcoin_tx.txid());

    println!("{}", serde_yaml::to_string(&tx.tx_type)?);

    Ok(())
}

fn process_satoshis(satoshis: Vec<u64>, required_length: usize) -> eyre::Result<Vec<u64>> {
    match satoshis.len() {
        len if len == required_length => Ok(satoshis),
        1 => Ok(vec![satoshis[0]; required_length]),
        _ => eyre::bail!("wrong number of 'satoshis' specified"),
    }
}
