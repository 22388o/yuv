use crate::context::Context;
use color_eyre::eyre;

pub async fn run(mut ctx: Context) -> eyre::Result<()> {
    let wallet = ctx.wallet().await?;

    let balances = wallet.balances();

    for (chroma, balance) in balances {
        println!(
            "{}: {}",
            chroma.to_address(ctx.config()?.network()),
            balance
        );
    }

    Ok(())
}
