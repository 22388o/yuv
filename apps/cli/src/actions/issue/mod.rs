use bitcoin::Amount;
use bitcoin_client::BitcoinRpcApi;
use clap::Args;
use color_eyre::eyre::{self, bail};
use yuv_pixels::Chroma;
use yuv_rpc_api::transactions::YuvTransactionsRpcClient;

use crate::{actions::transfer::process_satoshis, context::Context};

use super::rpc_args::RpcArgs;

pub const DEFAULT_SATOSHIS: u64 = 1000;

#[derive(Args, Debug)]
pub struct IssueArgs {
    /// Amount in satoshis that will be added to YUV UTXO.
    ///
    /// Default is 10,000 satoshis, if only one amount is provided it will be
    /// used for all recipients.
    #[clap(long, short, num_args = 1.., default_values_t = vec![DEFAULT_SATOSHIS])]
    pub satoshis: Vec<u64>,
    /// YUV token amount
    #[clap(long = "amount", num_args = 1..)]
    pub amounts: Vec<u128>,
    /// Public key of the recipient.
    #[clap(long = "recipient", num_args = 1.., value_parser = Chroma::from_address)]
    pub recipients: Vec<Chroma>,
    /// Provide proof of the transaction to YUV node.
    #[clap(long)]
    pub do_not_provide_proofs: bool,
    /// Drain tweaked satoshis to use for fees, instead of using regular satoshis.
    ///
    /// It's worth noting that change from regular satoshis will be tweaked.
    #[clap(long)]
    pub drain_tweaked_satoshis: bool,
    #[clap(flatten)]
    pub rpc_args: RpcArgs,
}

pub async fn run(
    IssueArgs {
        amounts,
        recipients,
        satoshis,
        rpc_args,
        do_not_provide_proofs,
        drain_tweaked_satoshis,
    }: IssueArgs,
    mut ctx: Context,
) -> eyre::Result<()> {
    if amounts.len() != recipients.len() {
        bail!("Amounts and recipients must have the same length");
    }

    let satoshis = process_satoshis(satoshis, amounts.len())?;

    let wallet = ctx.wallet().await?;
    let blockchain = ctx.blockchain()?;
    let bitcoin_client = ctx
        .bitcoin_client(rpc_args.rpc_url, rpc_args.rpc_auth, None)
        .await?;
    let cfg = ctx.config()?;

    let tx = {
        let mut builder = wallet.build_issuance()?;

        for ((recipient, amount), satoshis) in recipients.iter().zip(amounts).zip(satoshis) {
            builder.add_recipient(&recipient.public_key().inner, amount, satoshis);
        }

        builder
            .set_fee_rate_strategy(cfg.fee_rate_strategy)
            .set_drain_tweaked_satoshis(drain_tweaked_satoshis);

        builder.finish(&blockchain).await?
    };

    let tx_type = tx.tx_type.clone();
    let txid = bitcoin_client
        .send_raw_transaction_opts(
            &tx.bitcoin_tx,
            None,
            Some(Amount::from_sat(DEFAULT_SATOSHIS).to_btc()),
        )
        .await?;
    if !do_not_provide_proofs {
        let client = ctx.yuv_client()?;

        client.provide_yuv_proof(tx.clone()).await?;
    }

    println!("tx id: {}", txid);
    println!("{}", serde_yaml::to_string(&tx_type)?);

    Ok(())
}
