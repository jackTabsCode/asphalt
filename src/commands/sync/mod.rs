use self::state::SyncState;
use crate::{asset::Asset, cli::SyncArgs, util::svg::svg_to_png, FileEntry, LockFile};
use anyhow::Context;
use codegen::{generate_lua, generate_ts};
use config::SyncConfig;
use console::style;
use log::{debug, error, info};
use std::{collections::VecDeque, path::Path};
use tokio::fs::{read, read_dir, write, DirEntry};

mod codegen;
pub mod config;
mod state;

fn fix_path(path: &str) -> String {
    path.replace('\\', "/")
}

async fn process_file(entry: &DirEntry, state: &SyncState) -> anyhow::Result<Option<FileEntry>> {
    let path = entry.path();
    let path_str = path.to_str().unwrap();
    let fixed_path = fix_path(path_str);

    debug!("Processing {fixed_path}");

    let file_name = path
        .file_name()
        .with_context(|| format!("Failed to get file name of {}", fixed_path))?
        .to_str()
        .unwrap()
        .to_string();

    let mut bytes = read(&path)
        .await
        .with_context(|| format!("Failed to read {}", fixed_path))?;

    let mut extension = match path.extension().and_then(|s| s.to_str()) {
        Some(extension) => extension,
        None => return Ok(None),
    };

    if extension == "svg" {
        bytes = svg_to_png(&bytes, &state.font_db).await?;
        extension = "png";
    }

    let asset = Asset::new(file_name, bytes, extension)?;
    let hash = asset.hash();

    let existing = state.existing_lockfile.entries.get(fixed_path.as_str());

    if let Some(existing_value) = existing {
        if existing_value.hash == hash {
            return Ok(Some(FileEntry {
                hash,
                asset_id: existing_value.asset_id,
            }));
        }
    }

    let asset_id = asset
        .upload(state.creator.clone(), state.api_key.clone())
        .await
        .with_context(|| format!("Failed to upload {fixed_path}"))?;

    info!("Uploaded {}", style(fixed_path).green());

    Ok(Some(FileEntry { hash, asset_id }))
}

pub async fn sync(args: SyncArgs, existing_lockfile: LockFile) -> anyhow::Result<()> {
    let config = SyncConfig::read().await.context("Failed to read config")?;

    let mut state = SyncState::new(args, config, existing_lockfile)
        .await
        .context("Failed to create state")?;

    info!("Syncing...");

    let mut remaining_items = VecDeque::new();
    remaining_items.push_back(state.asset_dir.clone());

    while let Some(path) = remaining_items.pop_front() {
        let mut dir_entries = read_dir(path.clone())
            .await
            .with_context(|| format!("Failed to read directory: {}", path.to_str().unwrap()))?;

        while let Some(entry) = dir_entries
            .next_entry()
            .await
            .with_context(|| format!("Failed to read directory entry: {:?}", path))?
        {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                remaining_items.push_back(entry_path);
            } else {
                let path_str = entry_path.to_str().unwrap();
                let fixed_path = fix_path(path_str);

                let result = match process_file(&entry, &state).await {
                    Ok(Some(result)) => result,
                    Ok(None) => continue,
                    Err(e) => {
                        error!("Failed to process file {fixed_path}: {e:?}");
                        continue;
                    }
                };

                state.new_lockfile.entries.insert(fixed_path, result);
            }
        }
    }

    let _ = &state
        .new_lockfile
        .write()
        .await
        .context("Failed to write lockfile")?;

    let asset_dir_str = state.asset_dir.to_str().unwrap();

    state
        .new_lockfile
        .entries
        .extend(state.existing.into_iter().map(|(path, asset)| {
            (
                path,
                FileEntry {
                    hash: "".to_string(),
                    asset_id: asset.id,
                },
            )
        }));

    let lua_filename = format!("{}.{}", state.output_name, state.lua_extension);
    let lua_output = generate_lua(
        &state.new_lockfile,
        asset_dir_str,
        &state.style,
        state.strip_extension,
    );

    write(Path::new(&state.write_dir).join(lua_filename), lua_output?)
        .await
        .context("Failed to write output Lua file")?;

    if state.typescript {
        let ts_filename = format!("{}.d.ts", state.output_name);
        let ts_output = generate_ts(
            &state.new_lockfile,
            asset_dir_str,
            state.output_name.as_str(),
            &state.style,
            state.strip_extension,
        );

        write(Path::new(&state.write_dir).join(ts_filename), ts_output?)
            .await
            .context("Failed to write output TypeScript file")?;
    }

    info!("Synced!");

    Ok(())
}
