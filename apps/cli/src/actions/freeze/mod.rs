use bdk::blockchain::Blockchain;
use bitcoin::{OutPoint, Txid};
use clap::Args;
use color_eyre::eyre::{self, Context as EyreContext};

use crate::context::Context;

#[derive(Args, Debug)]
pub struct FreezeArgs {
    /// Transaction id
    pub txid: Txid,
    /// Output index
    pub vout: u32,
}
pub type UnfreezeArgs = FreezeArgs;

pub async fn run(args: FreezeArgs, mut context: Context) -> eyre::Result<()> {
    let blockchain = context.blockchain()?;
    let wallet = context.wallet().await?;

    let config = context.config()?;

    let outpoint = OutPoint::new(args.txid, args.vout);
    let yuv_tx = wallet
        .create_freeze(outpoint, config.fee_rate_strategy, &blockchain)
        .wrap_err("failed to create freeze transaction")?;

    blockchain.broadcast(&yuv_tx.bitcoin_tx)?;
    println!("Transaction broadcasted: {}", yuv_tx.bitcoin_tx.txid());

    Ok(())
}
