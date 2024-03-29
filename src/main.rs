use anyhow::Context;
use args::Args;
use blake3::Hasher;
use clap::Parser;
use codegen::{generate_lua, generate_ts};
use console::style;
use dotenv::dotenv;
pub use lockfile::{FileEntry, LockFile};
use rbxcloud::rbx::v1::assets::AssetType;
use state::State;
use std::{collections::VecDeque, path::Path};
use tokio::fs::{read, read_dir, write, DirEntry};
use upload::upload_asset;

pub mod args;
mod codegen;
pub mod lockfile;
pub mod state;
mod upload;

fn fix_path(path: &str) -> String {
    path.replace('\\', "/")
}

async fn check_file(entry: &DirEntry, state: &State) -> anyhow::Result<Option<FileEntry>> {
    let path = entry.path();
    let path_str = path.to_str().unwrap();
    let fixed_path = fix_path(path_str);

    let extension = match path.extension().and_then(|s| s.to_str()) {
        Some(extension) => extension,
        None => return Ok(None),
    };

    let asset_type = match AssetType::try_from_extension(extension) {
        Ok(asset_type) => asset_type,
        Err(e) => {
            eprintln!(
                "Skipping {} because it has an unsupported extension: {}",
                style(fixed_path).yellow(),
                e
            );
            return Ok(None);
        }
    };

    let mut hasher = Hasher::new();
    let bytes = read(&path).await.unwrap();
    hasher.update(&bytes);

    let hash = hasher.finalize().to_string();

    let existing = state.existing_lockfile.entries.get(fixed_path.as_str());

    if let Some(existing_value) = existing {
        if existing_value.hash == hash {
            return Ok(Some(FileEntry {
                hash,
                asset_id: existing_value.asset_id,
            }));
        }
    }

    let asset_id = upload_asset(
        path,
        asset_type,
        state.api_key.clone(),
        state.creator.clone(),
    )
    .await
    .with_context(|| format!("Failed to upload {}", fixed_path))?;

    eprintln!("Uploaded {}", style(fixed_path).green());

    Ok(Some(FileEntry { hash, asset_id }))
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let args = Args::parse();
    let mut state = State::new(args).await;

    eprintln!("{}", style("Syncing...").dim());

    let mut remaining_items = VecDeque::new();
    remaining_items.push_back(state.asset_dir.clone());

    while let Some(path) = remaining_items.pop_front() {
        let mut dir_entries = read_dir(path).await.expect("Failed to read directory");

        while let Some(entry) = dir_entries.next_entry().await.unwrap() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                remaining_items.push_back(entry_path);
            } else {
                let result = match check_file(&entry, &state).await {
                    Ok(Some(result)) => result,
                    Ok(None) => continue,
                    Err(e) => {
                        eprintln!("{} {:?}", style("Error:").red(), e);
                        continue;
                    }
                };

                let path_str = entry_path.to_str().unwrap();
                let fixed_path = fix_path(path_str);

                state.new_lockfile.entries.insert(fixed_path, result);
            }
        }
    }

    write(
        "asphalt.lock.toml",
        toml::to_string(&state.new_lockfile).unwrap(),
    )
    .await
    .expect("Failed to write lockfile");

    let asset_dir_str = state.asset_dir.to_str().unwrap();

    let lua_filename = format!("{}.{}", state.output_name, state.lua_extension);
    let lua_output = generate_lua(&state.new_lockfile, asset_dir_str);

    write(Path::new(&state.write_dir).join(lua_filename), lua_output)
        .await
        .expect("Failed to write output Lua file");

    if state.typescript {
        let ts_filename = format!("{}.d.ts", state.output_name);
        let ts_output = generate_ts(
            &state.new_lockfile,
            asset_dir_str,
            state.output_name.as_str(),
        );

        write(Path::new(&state.write_dir).join(ts_filename), ts_output)
            .await
            .expect("Failed to write output TypeScript file");
    }

    eprintln!("{}", style("Synced!").dim());
}
