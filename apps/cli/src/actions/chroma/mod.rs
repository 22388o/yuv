use crate::context::Context;
use clap::Subcommand;
use color_eyre::eyre;

mod announcement;
mod info;

#[derive(Subcommand, Debug)]
pub enum ChromaCommands {
    /// Make the Chroma announcement.
    Announcement(announcement::AnnnouncementArgs),
    /// Get the information about the token by its Chroma.
    Info(info::InfoArgs),
}

pub async fn run(cmd: ChromaCommands, context: Context) -> eyre::Result<()> {
    match cmd {
        ChromaCommands::Announcement(args) => announcement::run(args, context).await,
        ChromaCommands::Info(args) => info::run(args, context).await,
    }
}
