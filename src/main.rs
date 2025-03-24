use clap::Parser;
use cli::{Cli, Commands};
use dotenv::dotenv;
use log::LevelFilter;
use sync::sync;

mod asset;
mod cli;
mod config;
mod glob;
mod lockfile;
mod sync;
mod upload;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let args = Cli::parse();

    let logger = env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .filter_module("asphalt", args.verbose.log_level_filter())
        .format_timestamp(None)
        .format_module_path(false)
        .build();

    match args.command {
        Commands::Sync(args) => sync(logger, args).await,
    }
}
