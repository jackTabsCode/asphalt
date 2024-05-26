use clap::Parser;
use cli::{Cli, Commands};
use commands::{list::list, sync::sync};
use dotenv::dotenv;
pub use lockfile::{FileEntry, LockFile};
use tokio::fs::read_to_string;

pub mod cli;
mod commands;
pub mod lockfile;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let args = Cli::parse();

    let existing_lockfile: LockFile = toml::from_str(
        &read_to_string("asphalt.lock.toml")
            .await
            .unwrap_or_default(),
    )
    .unwrap_or_default();

    match args.command {
        Commands::Sync(sync_args) => sync(sync_args, existing_lockfile).await,
        Commands::List => list(existing_lockfile).await,
    }
}
