use clap::Parser;

#[derive(Parser, Debug)]
#[group(required = true, multiple = false)]
pub struct AssetCreatorGroup {
    /// A Roblox user ID
    #[arg(short, long)]
    pub user_id: Option<u64>,

    /// A Roblox group ID
    #[arg(short, long)]
    pub group_id: Option<u64>,
}

#[derive(Parser, Debug)]
#[command(version, about = "Sync assets to Roblox.")]
pub struct Args {
    // The directory of assets to upload to Roblox.
    #[arg(required = true)]
    pub asset_dir: String,

    /// The directory to write the output Luau (and optionally Typescript) files to.
    /// This should probably be somewhere in your game's source directory. This does not include the lockfile, which is always written to the current directory.
    #[arg(required = true)]
    pub write_dir: String,

    /// Your Open Cloud API key.
    /// Can also be set with the ASPHALT_API_KEY environment variable.
    #[arg(short, long)]
    pub api_key: Option<String>,

    /// Generate a TypeScript definition file for roblox-ts users.
    #[arg(short, long)]
    pub typescript: bool,

    #[clap(flatten)]
    pub creator: AssetCreatorGroup,

    #[arg(short, long)]
    pub output_name: Option<String>,

    #[arg(long)]
    pub luau: bool,
}
