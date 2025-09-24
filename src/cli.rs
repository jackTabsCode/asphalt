use crate::config::{CreatorType, PackAlgorithm, PackSort};
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

    /// Generate JSON schema for configuration files.
    GenerateSchema(GenerateSchemaArgs),
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

    /// Provides Roblox with the amount of Robux that you are willing to spend on each non-free asset upload.
    #[arg(long)]
    pub expected_price: Option<u32>,

    // Pack-related arguments
    /// Enable packing for all inputs that support it.
    #[arg(long)]
    pub pack: bool,

    /// Disable packing for all inputs.
    #[arg(long)]
    pub no_pack: bool,

    /// Maximum atlas size in format WxH (e.g., 2048x2048).
    #[arg(long, value_parser = parse_size)]
    pub pack_max_size: Option<(u32, u32)>,

    /// Padding between sprites in atlas.
    #[arg(long)]
    pub pack_padding: Option<u32>,

    /// Pixels to extrude sprite edges for filtering.
    #[arg(long)]
    pub pack_extrude: Option<u32>,

    /// Packing algorithm to use.
    #[arg(long)]
    pub pack_algorithm: Option<PackAlgorithm>,

    /// Enable sprite trimming to remove transparent borders.
    #[arg(long)]
    pub pack_trim: bool,

    /// Disable sprite trimming.
    #[arg(long)]
    pub pack_no_trim: bool,

    /// Maximum number of atlas pages to generate.
    #[arg(long)]
    pub pack_page_limit: Option<u32>,

    /// Sprite sorting method for deterministic packing.
    #[arg(long)]
    pub pack_sort: Option<PackSort>,

    /// Enable deduplication of identical sprites.
    #[arg(long)]
    pub pack_dedupe: bool,
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return Err("Size must be in format WxH (e.g., 2048x2048)".to_string());
    }

    let width = parts[0]
        .parse::<u32>()
        .map_err(|_| "Width must be a valid number")?;
    let height = parts[1]
        .parse::<u32>()
        .map_err(|_| "Height must be a valid number")?;

    Ok((width, height))
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

    /// Provides Roblox with the amount of Robux that you are willing to spend on each non-free asset upload.
    #[arg(long)]
    pub expected_price: Option<u32>,
}

#[derive(Args)]
pub struct MigrateLockfileArgs {
    /// The default input name to use. Only applies when upgrading from V0 to V1.
    pub input_name: Option<String>,
}

#[derive(Args)]
pub struct GenerateSchemaArgs {
    /// Output path for the JSON schema file.
    #[arg(short, long, default_value = ".schemas/asphalt.schema.json")]
    pub output: String,
}
