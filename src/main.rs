use anyhow::Context;
use clap::Parser;
use cli::{Cli, Commands};
use commands::{init::init, list::list, sync::sync};
use dotenv::dotenv;
pub use lockfile::{FileEntry, LockFile};
use log::LevelFilter;

pub mod asset;
pub mod cli;
mod commands;
pub mod lockfile;
pub mod upload;
pub mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let args = Cli::parse();

    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .filter_module("asphalt", args.verbose.log_level_filter())
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    let existing_lockfile = LockFile::read().await.context("Failed to read lockfile")?;

    match args.command {
        Commands::Sync(sync_args) => sync(sync_args, existing_lockfile)
            .await
            .context("Failed to sync"),
        Commands::List => list(existing_lockfile).await.context("Failed to list"),
        Commands::Init => init().await.context("Failed to initialize"),
        Commands::MigrateTarmacManifest(args) => {
            commands::migrate_tarmac_manifest::migrate_manifest(args)
                .await
                .context("Failed to migrate tarmac-manifest.toml")
        }
    }
}
