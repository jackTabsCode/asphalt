use crate::config::CreatorType;
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
    /// Sync assets.
    Sync(SyncArgs),

    /// Uploads a single asset and returns the asset ID.
    Upload(UploadArgs),

    /// Migrates a lockfile to the latest version.
    ///
    /// You can only run this once per upgrade, and it will overwrite the existing lockfile.
    /// Keep in mind that because pre-1.0 did not support multiple inputs, you'll need to provide a default input name for that migration.
    /// The pre-1.0 migration entails hashing your files again and updating the lockfile with the new hashes.
    /// We basically pretend nothing has changed, so your assets don't get reuploaded.
    MigrateLockfile(MigrateLockfileArgs),
}

#[derive(ValueEnum, Clone, Copy)]
pub enum SyncTarget {
    Cloud,
    Studio,
    Debug,
}

#[derive(Args, Clone)]
pub struct SyncArgs {
    /// Your Open Cloud API key.
    /// Can also be set with the ASPHALT_API_KEY environment variable.
    #[arg(short, long)]
    pub api_key: Option<String>,

    /// Where Asphalt should sync assets to.
    #[arg(short, long, default_value = "cloud")]
    pub target: SyncTarget,

    /// Skip asset syncing and only display what assets will be synced.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct UploadArgs {
    /// The file to upload.
    pub path: String,

    /// The creator type of the asset.
    #[arg(long)]
    pub creator_type: CreatorType,

    /// The creator ID of the asset.
    #[arg(long)]
    pub creator_id: u64,

    /// Your Open Cloud API key.
    /// Can also be set with the ASPHALT_API_KEY environment variable.
    #[arg(short, long)]
    pub api_key: Option<String>,

    /// Whether to alpha bleed if it's an image.
    #[arg(long, default_value = "true")]
    pub bleed: bool,

    /// Format the response as a link.
    #[arg(long)]
    pub link: bool,
}

#[derive(Args)]
pub struct MigrateLockfileArgs {
    /// The default input name to use. Only applies when upgrading from V0 to V1.
    pub input_name: Option<String>,
}
