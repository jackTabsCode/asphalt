use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Upload and reference Roblox assets in code.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Sync assets to Roblox.
    Sync(SyncArgs),
}

#[derive(Args)]
pub struct SyncArgs {
    /// Your Open Cloud API key.
    /// Can also be set with the ASPHALT_API_KEY environment variable.
    #[arg(short, long)]
    pub api_key: Option<String>,
}
