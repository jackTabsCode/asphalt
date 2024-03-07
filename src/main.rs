use clap::Parser;
use console::style;
use extension::FromExtension;
use rbxcloud::rbx::assets::AssetType;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use tokio::fs::{self, read};
use upload::upload_asset;

mod extension;
mod upload;

#[derive(Debug, Serialize, Deserialize)]
struct FileEntry {
    hash: String,
    asset_id: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct LockFile {
    entries: BTreeMap<String, FileEntry>,
}
#[derive(Parser, Debug)]
struct Args {
    // The directory of assets to look for
    #[arg(required = true)]
    read_directory: String,

    /// The directory to write the output to
    #[arg(required = true)]
    write_directory: String,

    /// Your Open Cloud API key
    #[arg(short, long)]
    api_key: String,

    /// Generate a TypeScript definition file
    #[arg(short, long)]
    typescript: bool,
}

const LOCKFILE_PATH: &str = "asphalt.lock.toml";

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let existing_lockfile: LockFile =
        toml::from_str(&fs::read_to_string(LOCKFILE_PATH).await.unwrap_or_default())
            .unwrap_or_default();

    let mut new_lockfile: LockFile = Default::default();

    let mut changed = false;

    let mut dir_entries = fs::read_dir(&args.read_directory)
        .await
        .expect("can't read dir");

    println!("{}", style("Syncing...").dim());

    while let Some(entry) = dir_entries.next_entry().await.unwrap() {
        let path = entry.path();
        let path_str = path.to_str().unwrap();

        let extension = path.extension().unwrap();
        let asset_type = match AssetType::from_extension(extension.to_str().unwrap()) {
            Some(asset_type) => asset_type,
            None => {
                println!("{} is not a supported file type!", style(path_str).red());
                continue;
            }
        };

        let mut hasher = blake3::Hasher::new();

        let bytes = read(&path).await.unwrap();
        hasher.update(&bytes);
        let hash = hasher.finalize().to_string();

        let mut asset_id: Option<u64> = None;

        let existing = existing_lockfile.entries.get(path_str);

        if let Some(existing_value) = existing {
            if existing_value.hash != hash || existing_value.asset_id.is_none() {
                changed = true;
            } else {
                asset_id = existing_value.asset_id;
            }
        } else {
            changed = true;
        }

        if asset_id.is_none() {
            asset_id = Some(upload_asset(path.clone(), asset_type, args.api_key.clone()).await);
            println!("Uploaded {}", style(path_str).green());
        }

        let entry_name = path_str.to_string();
        new_lockfile
            .entries
            .insert(entry_name, FileEntry { hash, asset_id });
    }

    if changed {
        fs::write(LOCKFILE_PATH, toml::to_string(&new_lockfile).unwrap())
            .await
            .expect("can't write lockfile");
    }

    let lua_table = new_lockfile
        .entries
        .iter()
        .map(|(file_name, file_entry)| {
            let file_stem = Path::new(file_name).file_stem().unwrap().to_str().unwrap();
            format!(
                "\t[\"{}\"] = \"rbxassetid://{}\"",
                file_stem,
                file_entry
                    .asset_id
                    .as_ref()
                    .expect("we never got an asset id?")
            )
        })
        .collect::<Vec<String>>()
        .join(",\n");

    let lua_output = format!("return {{\n{}\n}}", lua_table);

    let assets_lua_path = Path::new(&args.write_directory).join("assets.lua");
    fs::write(assets_lua_path, lua_output)
        .await
        .expect("can't write to assets.lua");

    if args.typescript {
        let ts_definitions = format!(
            "declare const assets: {{\n{}\n}}\nexport = assets",
            new_lockfile
                .entries
                .keys()
                .map(|file_name| {
                    let file_stem = Path::new(file_name).file_stem().unwrap().to_str().unwrap();
                    format!("\t\"{}\": string", file_stem)
                })
                .collect::<Vec<String>>()
                .join(",\n")
        );

        let assets_d_ts_path = Path::new(&args.write_directory).join("assets.d.ts");
        fs::write(assets_d_ts_path, ts_definitions)
            .await
            .expect("can't write to assets.d.ts");
    }

    println!("{}", style("Synced!").dim());
}
