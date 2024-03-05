use rbxcloud::rbx::assets::{
    create_asset, get_asset, AssetCreation, AssetCreationContext, AssetCreator, AssetType,
    AssetUserCreator, CreateAssetParams, GetAssetParams,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, read},
    path::PathBuf,
    time::Duration,
};

#[derive(Debug, Serialize, Deserialize)]
struct FileEntry {
    hash: String,
    asset_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LockFile {
    entries: BTreeMap<String, FileEntry>,
}

static EXTENSIONS: [&str; 3] = ["jpeg", "png", "jpg"];
static API_KEY: &str = "Pgq2mxqvjUSup1WReHIpep1amHq1/hb+Y8p2Fp+cV1n/mECa";

async fn upload_asset(path: PathBuf) -> String {
    let path_str = path.to_str().unwrap();

    let create_params = CreateAssetParams {
        api_key: API_KEY.to_string(),
        filepath: path_str.to_string(),
        asset: AssetCreation {
            asset_type: AssetType::DecalPng, // i've got to figure out the file extension mapping to the correct enum
            display_name: path_str.to_string(),
            creation_context: AssetCreationContext {
                creator: AssetCreator::User(AssetUserCreator {
                    user_id: "9670971".to_string(),
                }),
                expected_price: None,
            },
            description: "Hey".to_string(),
        },
    };
    let operation = create_asset(&create_params).await.unwrap();
    let id = operation
        .path
        .unwrap()
        .split_once("/")
        .unwrap()
        .1
        .to_string();

    let create_params = GetAssetParams {
        api_key: API_KEY.to_string(),
        operation_id: id,
    };

    // chatgpt v
    loop {
        match get_asset(&create_params).await {
            Ok(asset_operation) => {
                if let Some(done) = asset_operation.done {
                    if done {
                        return asset_operation.response.unwrap().asset_id;
                    }
                }
            }
            _ => {}
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // ^
}

#[tokio::main]
async fn main() {
    let existing_lockfile: LockFile =
        toml::from_str(&fs::read_to_string("test/asphault.lock.toml").unwrap_or("".to_string()))
            .unwrap_or(LockFile {
                entries: BTreeMap::new(),
            });
    // ^ this is defintiely bad

    let mut new_lockfile: LockFile = LockFile {
        entries: BTreeMap::new(),
    };

    let mut changed = false;

    let dir_entries = fs::read_dir("test").expect("can't read dir");
    for entry in dir_entries {
        let entry = entry.unwrap();
        let path = entry.path();

        let extension = path.extension().unwrap();
        if !EXTENSIONS.contains(&extension.to_str().unwrap()) {
            continue;
        }

        let mut hasher = blake3::Hasher::new();

        let bytes = read(&path).unwrap();
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
            asset_id = Some(upload_asset(path.clone()).await);
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
        .expect("can't write lockfile");

        println!("Synced")
    } else {
        println!("No changes")
    }
}
