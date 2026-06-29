use crate::config::{CreatorType, PackAlgorithm, PackSort};
use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use std::path::PathBuf;

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

    /// Generate shell completions for your shell.
    Completions(CompletionsArgs),

    /// Check configuration file for errors without syncing.
    Check(ProjectArgs),

    /// List assets that would be synced without actually syncing them.
    List(ProjectArgs),

    #[command(hide = true)]
    GenerateConfigSchema,
}

#[derive(Subcommand, Clone, Copy)]
pub enum SyncTarget {
    /// Upload assets to Roblox cloud.
    Cloud {
        /// Error if assets would be uploaded.
        #[arg(long)]
        dry_run: bool,
    },
    /// Write assets to the Roblox Studio content folder.
    Studio,
    /// Write assets to the .asphalt-debug folder.
    Debug,
}

impl SyncTarget {
    pub fn write_on_sync(&self) -> bool {
        matches!(self, SyncTarget::Cloud { dry_run: false })
    }
}

#[derive(Args, Clone)]
pub struct SyncArgs {
    /// Your Open Cloud API key.
    #[arg(short, long, env = "ASPHALT_API_KEY")]
    pub api_key: Option<String>,

    /// Where Asphalt should sync assets to.
    #[command(subcommand)]
    target: Option<SyncTarget>,

    /// Provides Roblox with the amount of Robux that you are willing to spend on each non-free asset upload.
    #[arg(long)]
    pub expected_price: Option<u32>,

    /// Path to the project directory. Defaults to the current directory.
    #[arg(short, long, default_value = ".")]
    pub project: PathBuf,

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

    /// Optimize PNG assets with oxipng for smaller file sizes.
    #[arg(long)]
    pub optimize: bool,
}

impl SyncArgs {
    pub fn target(&self) -> SyncTarget {
        self.target.unwrap_or(SyncTarget::Cloud { dry_run: false })
    }
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (width, height) = s
        .split_once('x')
        .ok_or_else(|| "Size must be in format WxH (e.g., 2048x2048)".to_string())?;

    Ok((
        width
            .parse::<u32>()
            .map_err(|_| "Width must be a valid number")?,
        height
            .parse::<u32>()
            .map_err(|_| "Height must be a valid number")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_valid() {
        let result = parse_size("2048x2048").unwrap();
        assert_eq!(result, (2048, 2048));
    }

    #[test]
    fn test_parse_size_varied_dimensions() {
        let result = parse_size("512x1024").unwrap();
        assert_eq!(result, (512, 1024));
    }

    #[test]
    fn test_parse_size_minimal() {
        let result = parse_size("1x1").unwrap();
        assert_eq!(result, (1, 1));
    }

    #[test]
    fn test_parse_size_large() {
        let result = parse_size("8192x4096").unwrap();
        assert_eq!(result, (8192, 4096));
    }

    #[test]
    fn test_parse_size_invalid_no_x() {
        let err = parse_size("2048").unwrap_err();
        assert!(err.contains("WxH"));
    }

    #[test]
    fn test_parse_size_invalid_empty() {
        let err = parse_size("").unwrap_err();
        assert!(err.contains("WxH"));
    }

    #[test]
    fn test_parse_size_invalid_non_numeric() {
        let err = parse_size("abcxdef").unwrap_err();
        assert!(err.contains("Width") || err.contains("number"));
    }

    #[test]
    fn test_parse_size_invalid_negative() {
        let err = parse_size("-100x100").unwrap_err();
        assert!(err.contains("Width") || err.contains("number"));
    }

    #[test]
    fn test_sync_target_cloud_write_on_sync() {
        let target = SyncTarget::Cloud { dry_run: false };
        assert!(target.write_on_sync());
    }

    #[test]
    fn test_sync_target_cloud_dry_run_no_write() {
        let target = SyncTarget::Cloud { dry_run: true };
        assert!(!target.write_on_sync());
    }

    #[test]
    fn test_sync_target_studio_no_write() {
        let target = SyncTarget::Studio;
        assert!(!target.write_on_sync());
    }

    #[test]
    fn test_sync_target_debug_no_write() {
        let target = SyncTarget::Debug;
        assert!(!target.write_on_sync());
    }
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
    #[arg(short, long, env = "ASPHALT_API_KEY")]
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

    /// Path to the project directory. Defaults to the current directory.
    #[arg(short, long, default_value = ".")]
    pub project: PathBuf,
}

#[derive(Args)]
pub struct GenerateSchemaArgs {
    /// Output path for the JSON schema file.
    #[arg(short, long, default_value = ".schemas/asphalt.schema.json")]
    pub output: String,
}

#[derive(Args)]
pub struct CompletionsArgs {
    /// The shell to generate completions for.
    pub shell: Shell,
}

#[derive(Args)]
pub struct ProjectArgs {
    /// Path to the project directory. Defaults to the current directory.
    #[arg(short, long, default_value = ".")]
    pub project: PathBuf,
}
