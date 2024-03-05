use clap::Parser;
use extension::FromExtension;
use rbxcloud::rbx::assets::{
    create_asset, get_asset, AssetCreation, AssetCreationContext, AssetCreator, AssetType,
    AssetUserCreator, CreateAssetParams, GetAssetParams,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
use tokio::fs::{self, read};
use upload::upload_asset;

mod extension;
mod upload;

#[derive(Debug, Serialize, Deserialize)]
struct FileEntry {
    hash: String,
    asset_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct LockFile {
    entries: BTreeMap<String, FileEntry>,
}

const API_KEY: &str = "Pgq2mxqvjUSup1WReHIpep1amHq1/hb+Y8p2Fp+cV1n/mECa";

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    api_key: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let existing_lockfile: LockFile = toml::from_str(
        &fs::read_to_string("test/asphault.lock.toml")
            .await
            .unwrap_or_default(),
    )
    .unwrap_or_default();

    let mut new_lockfile: LockFile = Default::default();

    let mut changed = false;

    let mut dir_entries = fs::read_dir("test").await.expect("can't read dir");
    while let Some(entry) = dir_entries.next_entry().await.unwrap() {
        let path = entry.path();

        let extension = path.extension().unwrap();
        let asset_type = match AssetType::from_extension(extension.to_str().unwrap()) {
            Some(asset_type) => asset_type,
            None => {
                println!("{} is not a supported file type", path.to_str().unwrap());
                continue;
            }
        };

        let mut hasher = blake3::Hasher::new();

        let bytes = read(&path).await.unwrap();
        hasher.update(&bytes);
        let hash = hasher.finalize().to_string();

        let mut asset_id: Option<String> = None;

        let existing = existing_lockfile.entries.get(path.to_str().unwrap());

        if let Some(existing_value) = existing {
            if existing_value.hash != hash || existing_value.asset_id.is_none() {
                changed = true;
                println!("{} is out of date", path.to_str().unwrap());
            } else {
                asset_id = existing_value.asset_id.clone();
            }
        } else {
            changed = true;
            println!("{} is new", path.to_str().unwrap());
        }

        if asset_id.is_none() {
            asset_id = Some(upload_asset(path.clone(), asset_type, args.api_key.clone()).await);
            println!("Uploaded asset: {:?}", asset_id.clone().unwrap());
        }

        let entry_name = path.to_str().unwrap().to_string();
        new_lockfile
            .entries
            .insert(entry_name, FileEntry { hash, asset_id });
    }

    if changed {
        fs::write(
            "test/asphault.lock.toml",
            toml::to_string(&new_lockfile).unwrap(),
        )
        .await
        .expect("can't write lockfile");

        println!("Synced");
    } else {
        println!("No changes");
    }
}
