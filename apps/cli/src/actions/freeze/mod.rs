use bitcoin::{Amount, OutPoint, Txid};
use clap::Args;
use color_eyre::eyre::{self, bail, Context as EyreContext};

use crate::context::Context;
use bitcoin_client::BitcoinRpcApi;

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

/// The version of Bitcoin Core RPC v24.0
const BITCOIN_CORE_RPC_V24: usize = 249999;

/// The version of Bitcoin Core RPC v25.0
const BITCOIN_CORE_RPC_V25: usize = 250000;

pub async fn run(args: FreezeArgs, mut context: Context) -> eyre::Result<()> {
    let blockchain = context.blockchain()?;
    let wallet = context.wallet().await?;

    let config = context.config()?;

    // NOTE: Esplora doesn't support the max_burn_amount option. Therefore, to carry out Freeze &
    // Unfreeze operations, the Bitcoin Core RPC needs to be specified. The Esplora will be used to
    // sync the wallet transactions.
    let bitcoin_client = context
        .bitcoin_client(args.rpc_args.rpc_url, args.rpc_args.rpc_auth, None)
        .await?;

    let outpoint = OutPoint::new(args.txid, args.vout);
    let tx = wallet
        .create_freeze(
            outpoint,
            config.fee_rate_strategy,
            &blockchain,
            args.satoshis,
        )
        .wrap_err("failed to create freeze transaction")?;

    let txid = match bitcoin_client.get_network_info().await?.version {
        ..=BITCOIN_CORE_RPC_V24 => bitcoin_client.send_raw_transaction(&tx.bitcoin_tx).await?,
        BITCOIN_CORE_RPC_V25.. => {
            bitcoin_client
                .send_raw_transaction_opts(
                    &tx.bitcoin_tx,
                    None,
                    Amount::from_sat(args.satoshis).to_btc(),
                )
                .await?
        }
        _ => bail!("Failed to determine bitcoin rpc network version"),
    };

    println!("Transaction broadcasted: {}", txid);

    Ok(())
}
