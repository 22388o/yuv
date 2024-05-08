use bitcoin::{Amount, OutPoint, Txid};
use bitcoin_client::BitcoinRpcApi;
use clap::Args;
use color_eyre::eyre::{self, Context as EyreContext};

use crate::context::Context;

use super::rpc_args::RpcArgs;

#[derive(Args, Debug)]
pub struct FreezeArgs {
    /// Satoshis
    #[clap(long, short, default_value_t = 1000)]
    pub satoshis: u64,
    /// Transaction id
    pub txid: Txid,
    /// Output index
    pub vout: u32,
    /// Rpc connection arguments
    #[clap(flatten)]
    pub rpc_args: RpcArgs,
}
pub type UnfreezeArgs = FreezeArgs;

pub async fn run(args: FreezeArgs, mut context: Context) -> eyre::Result<()> {
    let blockchain = context.blockchain()?;
    let bitcoin_client = context
        .bitcoin_client(args.rpc_args.rpc_url, args.rpc_args.rpc_auth, None)
        .await?;
    let wallet = context.wallet().await?;

    let config = context.config()?;

    let outpoint = OutPoint::new(args.txid, args.vout);
    let yuv_tx = wallet
        .create_freeze(
            outpoint,
            config.fee_rate_strategy,
            &blockchain,
            args.satoshis,
        )
        .wrap_err("failed to create freeze transaction")?;

    let txid = bitcoin_client
        .send_raw_transaction_opts(
            &yuv_tx.bitcoin_tx,
            None,
            Some(Amount::from_sat(args.satoshis).to_btc()),
        )
        .await?;
    println!("Transaction broadcasted: {}", txid);

    Ok(())
}
