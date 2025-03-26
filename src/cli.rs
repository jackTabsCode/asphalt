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

    /// Migrates a lockfile from pre-1.0 to 1.0. You can only run this once, and it will overwrite the existing lockfile.
    /// Keep in mind that because pre-1.0 did not support multiple inputs, you'll need to provide a default input name.
    /// The migration entails hashing your files again and updating the lockfile with the new hashes.
    /// We basically pretend nothing has changed, so your assets don't get reuploaded.
    MigrateLockfile(MigrateLockfileArgs),
}

#[derive(ValueEnum, Clone)]
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

    /// Your cookie.
    /// This is only required if you are uploading animations with Asphalt.
    #[arg(long)]
    pub cookie: Option<String>,

    /// Where Asphalt should sync assets to.
    #[arg(short, long, default_value = "cloud")]
    pub target: SyncTarget,

    /// Skip asset syncing and only display what assets will be synced.
    #[arg(long, action)]
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

    /// Your cookie.
    /// This is only required if you are uploading animations with Asphalt.
    #[arg(long)]
    pub cookie: Option<String>,

    /// Whether to alpha bleed if it's an image.
    #[arg(long, default_value = "true")]
    pub bleed: bool,

    /// Format it as a link instead of just the asset ID.
    #[arg(long)]
    pub link: bool,
}

#[derive(Args)]
pub struct MigrateLockfileArgs {
    /// The default input name to use.
    pub input_name: String,
}
