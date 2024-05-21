use crate::context::Context;
use bdk::blockchain::Blockchain;
use clap::Args;
use color_eyre::eyre::{self, Context as EyreContext};
use yuv_pixels::Chroma;
use yuv_types::announcements::ChromaAnnouncement;

/// Arguments to make a chroma announcement. See [`ChromaAnnouncement`].
#[derive(Clone, Args, Debug)]
pub struct AnnnouncementArgs {
    /// The [`Chroma`] to announce.
    #[clap(long, short, value_parser = Chroma::from_address)]
    pub chroma: Option<Chroma>,
    /// The name of the token.
    #[clap(long, short)]
    pub name: String,
    /// The symbol of the token.
    #[clap(long)]
    pub symbol: String,
    /// The decimals of the token.
    #[clap(long, short, default_value_t = 0)]
    pub decimal: u8,
    /// The maximum supply of the token. 0 - supply is unlimited.
    #[clap(long, default_value_t = 0)]
    pub max_supply: u128,
    /// Indicates whether the token can be frozen by the issuer.
    #[clap(long, default_value_t = true)]
    pub is_freezable: bool,
}

impl AnnnouncementArgs {
    pub fn try_into_announcement(self, chroma: Chroma) -> eyre::Result<ChromaAnnouncement> {
        Ok(ChromaAnnouncement::new(
            chroma,
            self.name,
            self.symbol,
            self.decimal,
            self.max_supply,
            self.is_freezable,
        )?)
    }
}

pub async fn run(args: AnnnouncementArgs, mut context: Context) -> eyre::Result<()> {
    let blockchain = context.blockchain()?;

    let wallet = context.wallet().await?;
    let config = context.config()?;

    let chroma = args.chroma.unwrap_or(Chroma::from(wallet.public_key()));

    let announcement = args.clone().try_into_announcement(chroma)?;

    let yuv_tx = wallet
        .create_announcement_tx(announcement.into(), config.fee_rate_strategy, &blockchain)
        .wrap_err("failed to create chroma announcement tx")?;

    blockchain
        .broadcast(&yuv_tx.bitcoin_tx)
        .wrap_err("failed to broadcast tx")?;

    println!("Transaction broadcasted: {}", yuv_tx.bitcoin_tx.txid());

    Ok(())
}
