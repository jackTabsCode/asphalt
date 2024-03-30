use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about = "Sync assets to Roblox.")]
pub struct Args {
    /// Your Open Cloud API key.
    /// Can also be set with the ASPHALT_API_KEY environment variable.
    #[arg(short, long)]
    pub api_key: Option<String>,
}
