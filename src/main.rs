use async_recursion::async_recursion;
use clap::Parser;
use codegen::{generate_lua, generate_ts};
use console::style;
use dotenv::dotenv;
pub use lockfile::{FileEntry, LockFile};
use rbxcloud::rbx::assets::{AssetCreator, AssetGroupCreator, AssetType, AssetUserCreator};
use std::path::Path;
use tokio::fs::{self, read, DirEntry};
use upload::upload_asset;

mod codegen;
pub mod lockfile;
mod upload;

#[derive(Parser, Debug)]
#[group(required = true, multiple = false)]
struct AssetCreatorGroup {
    /// A Roblox user ID
    #[arg(short, long)]
    user_id: Option<u64>,

    /// A Roblox group ID
    #[arg(short, long)]
    group_id: Option<u64>,
}

#[derive(Parser, Debug)]
#[command(version, about = "Sync assets to Roblox.")]
struct Args {
    // The directory of assets to upload to Roblox.
    #[arg(required = true)]
    asset_dir: String,

    /// The directory to write the output Luau (and optionally Typescript) files to.
    /// This should probably be somewhere in your game's source directory. This does not include the lockfile, which is always written to the current directory.
    #[arg(required = true)]
    write_dir: String,

    /// Your Open Cloud API key.
    /// Can also be set with the ASPHALT_API_KEY environment variable.
    #[arg(short, long)]
    api_key: Option<String>,

    /// Generate a TypeScript definition file for roblox-ts users.
    #[arg(short, long)]
    typescript: bool,

    #[clap(flatten)]
    creator: AssetCreatorGroup,

    #[arg(short, long)]
    output_name: Option<String>,
}

const LOCKFILE_PATH: &str = "asphalt.lock.toml";

fn fix_path(path: &str) -> String {
    path.replace("\\", "/")
}

async fn handle_file_entry(
    entry: &DirEntry,
    existing_lockfile: &LockFile,
    creator: &AssetCreatorGroup,
    api_key: &str,
) -> Option<FileEntry> {
    let path = entry.path();
    let path_str = path.to_str().unwrap();
    let fixed_path = fix_path(path_str);

    let extension = path.extension().and_then(|s| s.to_str())?;

    let asset_type = match AssetType::try_from_extension(extension) {
        Ok(asset_type) => asset_type,
        Err(e) => {
            eprintln!(
                "Skipping {} because it has an unsupported extension: {}",
                style(fixed_path).yellow(),
                e
            );
            return None;
        }
    };

    let mut hasher = blake3::Hasher::new();

    let bytes = read(&path).await.unwrap();
    hasher.update(&bytes);
    let hash = hasher.finalize().to_string();

    let mut asset_id: Option<u64> = None;

    let existing = existing_lockfile.entries.get(fixed_path.as_str());

    if let Some(existing_value) = existing {
        if existing_value.hash != hash || existing_value.asset_id.is_none() {
        } else {
            asset_id = existing_value.asset_id;
        }
    }

    let asset_creator: AssetCreator = match creator {
        AssetCreatorGroup {
            user_id: Some(user_id),
            group_id: None,
        } => AssetCreator::User(AssetUserCreator {
            user_id: user_id.to_string(),
        }),
        AssetCreatorGroup {
            user_id: None,
            group_id: Some(group_id),
        } => AssetCreator::Group(AssetGroupCreator {
            group_id: group_id.to_string(),
        }),
        _ => return None,
    };

    if asset_id.is_none() {
        asset_id =
            Some(upload_asset(path.clone(), asset_type, api_key.to_string(), asset_creator).await);
        println!("Uploaded {}", style(fixed_path).green());
    }

    Some(FileEntry { hash, asset_id })
}

#[async_recursion]
async fn traverse_dir(
    dir_path: &Path,
    existing_lockfile: &LockFile,
    creator: &AssetCreatorGroup,
    api_key: &str,
    new_lockfile: &mut LockFile,
) {
    let mut dir_entries = fs::read_dir(dir_path).await.expect("can't read dir");

    while let Some(entry) = dir_entries.next_entry().await.unwrap() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            traverse_dir(
                &entry_path,
                existing_lockfile,
                creator,
                api_key,
                new_lockfile,
            )
            .await;
        } else if let Some(result) =
            handle_file_entry(&entry, existing_lockfile, creator, api_key).await
        {
            let path_str = entry_path.to_str().unwrap();
            let fixed_path = fix_path(path_str);

            new_lockfile.entries.insert(fixed_path, result);
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    dotenv().ok();

    let api_key: String = args
        .api_key
        .unwrap_or_else(|| std::env::var("ASPHALT_API_KEY").expect("no API key provided"));

    let output_name = args.output_name.unwrap_or("assets".to_string());

    fs::create_dir_all(&args.write_dir)
        .await
        .expect("can't create write dir");

    let existing_lockfile: LockFile =
        toml::from_str(&fs::read_to_string(LOCKFILE_PATH).await.unwrap_or_default())
            .unwrap_or_default();

    let mut new_lockfile: LockFile = Default::default();

    println!("{}", style("Syncing...").dim());

    let asset_dir_path = Path::new(&args.asset_dir);
    traverse_dir(
        asset_dir_path,
        &existing_lockfile,
        &args.creator,
        &api_key,
        &mut new_lockfile,
    )
    .await;

    let asset_directory_path_str = asset_dir_path.to_str().unwrap();

    fs::write(LOCKFILE_PATH, toml::to_string(&new_lockfile).unwrap())
        .await
        .expect("can't write lockfile");

    let lua_filename = format!("{}.lua", output_name);
    let lua_output = generate_lua(&new_lockfile, asset_directory_path_str);

    fs::write(Path::new(&args.write_dir).join(lua_filename), lua_output)
        .await
        .expect("can't write output lua file");

    if args.typescript {
        let ts_filename = format!("{}.d.ts", output_name);
        let ts_output = generate_ts(
            &new_lockfile,
            asset_directory_path_str,
            output_name.as_str(),
        );

        fs::write(Path::new(&args.write_dir).join(ts_filename), ts_output)
            .await
            .expect("can't write output ts file");
    }

    println!("{}", style("Synced!").dim());
}
