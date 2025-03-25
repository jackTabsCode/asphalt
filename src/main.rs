use clap::Parser;
use cli::{Cli, Commands};
use dotenv::dotenv;
use log::LevelFilter;
use sync::sync;
use upload_command::upload;

mod asset;
mod auth;
mod cli;
mod config;
mod glob;
mod lockfile;
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
        .format_module_path(false);

    if !matches!(args.command, Commands::Sync(_)) {
        logger.init();
    }

    match args.command {
        Commands::Sync(args) => sync(logger.build(), args).await,
        Commands::Upload(args) => upload(args).await,
    }
}
