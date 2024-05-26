use clap::Parser;
use cli::{Cli, Commands};
use commands::sync::sync;
use dotenv::dotenv;
pub use lockfile::{FileEntry, LockFile};

pub mod cli;
mod commands;
pub mod lockfile;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let args = Cli::parse();

    match args.command {
        Commands::Sync(sync_args) => sync(sync_args).await,
    }
}
