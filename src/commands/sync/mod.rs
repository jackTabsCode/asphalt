use self::state::SyncState;
use crate::{
    asset::Asset,
    cli::{SyncArgs, SyncTarget},
    FileEntry, LockFile,
};
use anyhow::Context;
use backend::{
    cloud::CloudBackend, debug::DebugBackend, studio::StudioBackend, SyncBackend, SyncResult,
};
use codegen::{generate_lua, generate_ts};
use config::SyncConfig;
use log::{debug, info, warn};
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
};
use tokio::fs::{read, read_dir, write, DirEntry};

mod backend;
mod codegen;
pub mod config;
mod state;

fn fix_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn format_asset_id(asset_id: u64) -> String {
    format!("rbxassetid://{}", asset_id)
}

enum TargetBackend {
    Cloud(CloudBackend),
    Studio(StudioBackend),
    Debug(DebugBackend),
}

struct ProcessResult {
    asset_id: String,
    file_entry: Option<FileEntry>,
}

async fn process_file(
    entry: &DirEntry,
    state: &mut SyncState,
    backend: &TargetBackend,
) -> anyhow::Result<Option<ProcessResult>> {
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

    let data = read(&path)
        .await
        .with_context(|| format!("Failed to read {}", fixed_path))?;

    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(extension) => extension,
        None => {
            warn!("Failed to get extension of {fixed_path}");
            return Ok(None);
        }
    };

    let asset = Asset::new(file_name, data, ext, &state.font_db).await?;
    let hash = asset.hash();

    let existing = state.existing_lockfile.entries.get(fixed_path.as_str());

    if let Some(existing_value) = existing {
        if matches!(state.target, SyncTarget::Cloud) && existing_value.hash == hash {
            return Ok(Some(ProcessResult {
                asset_id: format_asset_id(existing_value.asset_id),
                file_entry: Some(FileEntry {
                    hash,
                    asset_id: existing_value.asset_id,
                }),
            }));
        }
    }

    if state.dry_run {
        info!("Sync {fixed_path}");
        return Ok(None);
    }

    let sync_result = match &backend {
        TargetBackend::Cloud(backend) => backend.sync(state, &fixed_path, asset).await,
        TargetBackend::Studio(backend) => backend.sync(state, &fixed_path, asset).await,
        TargetBackend::Debug(backend) => backend.sync(state, &fixed_path, asset).await,
    }
    .with_context(|| format!("Failed to sync {fixed_path}"))?;

    match sync_result {
        SyncResult::Cloud(asset_id) => Ok(Some(ProcessResult {
            asset_id: format_asset_id(asset_id),
            file_entry: Some(FileEntry { hash, asset_id }),
        })),
        SyncResult::Studio(asset_id) => Ok(Some(ProcessResult {
            asset_id,
            file_entry: None,
        })),
        SyncResult::None => Ok(None),
    }
}

pub async fn sync(args: SyncArgs, existing_lockfile: LockFile) -> anyhow::Result<()> {
    let config = SyncConfig::read().await.context("Failed to read config")?;

    let mut state = SyncState::new(args, config, existing_lockfile)
        .await
        .context("Failed to create state")?;

    info!("Syncing...");

    let mut assets = BTreeMap::<String, String>::new();
    let mut remaining_items = VecDeque::new();
    remaining_items.push_back(state.asset_dir.clone());

    let backend = match state.target {
        SyncTarget::Cloud => TargetBackend::Cloud(CloudBackend),
        SyncTarget::Studio => TargetBackend::Studio(StudioBackend::new()?),
        SyncTarget::Debug => TargetBackend::Debug(DebugBackend),
    };

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

                if state.exclude_assets_matcher.is_match(path_str) {
                    continue;
                }

                let result = match process_file(&entry, &mut state, &backend).await {
                    Ok(Some(result)) => result,
                    Ok(None) => continue,
                    Err(e) => {
                        warn!("Failed to process file {fixed_path}: {e:?}");
                        continue;
                    }
                };

                assets.insert(fixed_path.clone(), result.asset_id);
                if let Some(file_entry) = result.file_entry {
                    state.new_lockfile.entries.insert(fixed_path, file_entry);
                }
            }
        }
    }

    if state.dry_run || matches!(state.target, SyncTarget::Debug) {
        info!("Synced!");
        return Ok(());
    }

    if let SyncTarget::Cloud = state.target {
        state
            .new_lockfile
            .write()
            .await
            .context("Failed to write lockfile")?;
    }

    assets.extend(
        state
            .existing
            .into_iter()
            .map(|(path, asset)| (path, format_asset_id(asset.id))),
    );

    let asset_dir_str = state.asset_dir.to_str().unwrap();
    let lua_filename = format!("{}.{}", state.output_name, state.lua_extension);
    let lua_output = generate_lua(&assets, asset_dir_str, &state.style, state.strip_extension);

    write(Path::new(&state.write_dir).join(lua_filename), lua_output?)
        .await
        .context("Failed to write output Lua file")?;

    if state.typescript {
        let ts_filename = format!("{}.d.ts", state.output_name);
        let ts_output = generate_ts(
            &assets,
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
