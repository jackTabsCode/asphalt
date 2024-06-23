use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_verbosity_flag::{InfoLevel, Verbosity};

#[derive(Parser)]
#[command(version, about = "Upload and reference Roblox assets in code.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Sync assets to Roblox.
    Sync(SyncArgs),

    /// List assets in the lockfile.
    List,

    /// Initialize a new configuration.
    Init,
}

#[derive(ValueEnum, Clone)]
pub enum SyncTarget {
    Roblox,
    Local,
    Debug,
}

#[derive(Args)]
pub struct SyncArgs {
    /// Your Open Cloud API key.
    /// Can also be set with the ASPHALT_API_KEY environment variable.
    #[arg(short, long)]
    pub api_key: Option<String>,

    /// Your cookie.
    /// This is only required if you are uploading animations with Asphalt.
    #[arg(long)]
    pub cookie: Option<String>,

    /// Where Asphalt should sync assets to.
    #[arg(short, long)]
    pub target: Option<SyncTarget>,

    /// Skip asset syncing and only display what assets will be synced.
    #[arg(long, action)]
    pub dry_run: bool,
}
