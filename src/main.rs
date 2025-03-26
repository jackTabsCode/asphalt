use clap::Parser;
use cli::{Cli, Commands};
use dotenv::dotenv;
use indicatif::MultiProgress;
use log::LevelFilter;
use migrate_lockfile::migrate_lockfile;
use sync::sync;
use upload_command::upload;

mod asset;
mod auth;
mod cli;
mod config;
mod glob;
mod lockfile;
mod migrate_lockfile;
mod sync;
mod upload;
mod upload_command;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let args = Cli::parse();

    let mut binding = env_logger::Builder::new();
    let logger = binding
        .filter_level(LevelFilter::Info)
        .filter_module("asphalt", args.verbose.log_level_filter())
        .format_timestamp(None)
        .format_module_path(false)
        .build();

    let level = logger.filter();

    let multi_progress = MultiProgress::new();
    indicatif_log_bridge::LogWrapper::new(multi_progress.clone(), logger).try_init()?;

    log::set_max_level(level);

    match args.command {
        Commands::Sync(args) => sync(multi_progress, args).await,
        Commands::Upload(args) => upload(args).await,
        Commands::MigrateLockfile(args) => migrate_lockfile(args).await,
    }
}
