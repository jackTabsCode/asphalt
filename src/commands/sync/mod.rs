use self::state::SyncState;
use crate::{
    asset::Asset,
    cli::{SyncArgs, SyncTarget},
    lockfile::SpriteInfo,
    util::spritesheet,
    FileEntry, LockFile,
};
use anyhow::{anyhow, Context};
use backend::{
    cloud::CloudBackend, debug::DebugBackend, studio::StudioBackend, SyncBackend, SyncResult,
};
use codegen::{generate_luau, generate_ts, AssetValue};
use config::SyncConfig;
use image::DynamicImage;
use log::{debug, info, warn};
use std::{
    collections::{BTreeMap, HashSet},
    io::Cursor,
    path::{Path, PathBuf},
};
use tokio::fs;
use walkdir::{DirEntry, WalkDir};

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
    asset_value: AssetValue,
    file_entry: Option<FileEntry>,
}

#[derive(Default)]
struct DryRunStats {
    pending_files: Vec<String>,
    pending_spritesheets: Vec<String>,
    pending_sprite_count: usize,
}

impl DryRunStats {
    fn has_pending_assets(&self) -> bool {
        !self.pending_files.is_empty() || !self.pending_spritesheets.is_empty()
    }

    fn report(&self) {
        if !self.pending_files.is_empty() {
            info!("Files that need uploading ({}):", self.pending_files.len());
            for file in &self.pending_files {
                info!("  - {}", file);
            }
        }

        if !self.pending_spritesheets.is_empty() {
            info!(
                "Spritesheets that need uploading ({} with {} sprites):",
                self.pending_spritesheets.len(),
                self.pending_sprite_count
            );
            info!("{}", self.pending_spritesheets.join(", "));
        }

        if !self.has_pending_assets() {
            info!("All assets are already in sync!");
        }
    }
}

async fn process_file(
    entry: &DirEntry,
    state: &mut SyncState,
    backend: &TargetBackend,
    dry_run_stats: &mut Option<DryRunStats>,
) -> anyhow::Result<Option<ProcessResult>> {
    let path = entry.path();
    let path_str = path.to_str().unwrap();
    let fixed_path = fix_path(path_str);

    debug!("Processing {fixed_path}");

    if state.is_in_spritesheet(&fixed_path) {
        debug!("Skipping {fixed_path} as it will be included in a spritesheet");
        return Ok(None);
    }

    let file_name = path.file_name().unwrap().to_str().unwrap().to_string();

    let data = fs::read(&path)
        .await
        .with_context(|| format!("Failed to read {}", fixed_path))?;

    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(extension) => extension,
        None => {
            warn!("Failed to get extension of {fixed_path}");
            return Ok(None);
        }
    };

    let asset = Asset::new(file_name, data, ext, state.fontdb.clone(), true).await?;
    let hash = asset.hash();

    let needs_sync = match state.existing_lockfile.entries.get(&fixed_path) {
        Some(entry) => entry.hash.as_ref() != Some(&hash),
        None => true,
    };

    if !needs_sync {
        debug!("Skipping {fixed_path} as it hasn't changed");
        return Ok(None);
    }

    if state.dry_run {
        if let Some(stats) = dry_run_stats {
            stats.pending_files.push(fixed_path.clone());
        }
        info!("Would sync {fixed_path}");
        return Ok(None);
    }

    let sync_result = match &backend {
        TargetBackend::Cloud(backend) => backend.sync(state, &fixed_path, &asset).await,
        TargetBackend::Studio(backend) => backend.sync(state, &fixed_path, &asset).await,
        TargetBackend::Debug(backend) => backend.sync(state, &fixed_path, &asset).await,
    }
    .with_context(|| format!("Failed to sync {fixed_path}"))?;

    match sync_result {
        SyncResult::Cloud(asset_id) => {
            let asset_id_str = format_asset_id(asset_id);

            Ok(Some(ProcessResult {
                asset_value: AssetValue::Asset(asset_id_str),
                file_entry: Some(FileEntry {
                    hash: Some(hash),
                    asset_id,
                    sprite: None,
                }),
            }))
        }
        SyncResult::Studio(asset_id) => Ok(Some(ProcessResult {
            asset_value: AssetValue::Asset(asset_id),
            file_entry: None,
        })),
        SyncResult::None => Ok(None),
    }
}

async fn process_spritesheets(
    state: &mut SyncState,
    backend: &TargetBackend,
    dry_run_stats: &mut Option<DryRunStats>,
) -> anyhow::Result<BTreeMap<String, AssetValue>> {
    let asset_dir = PathBuf::from(&state.asset_dir);

    let images = spritesheet::collect_images_for_packing(
        &asset_dir,
        &state.spritesheet_matcher,
        &state.exclude_assets_matcher,
        state.fontdb.clone(),
    )
    .await?;

    if images.is_empty() {
        return Ok(BTreeMap::new());
    }

    let mut sprite_assets = BTreeMap::new();

    let dir_name = "spritesheet";

    let spritesheets = spritesheet::pack_spritesheets(&images)?;

    for (sheet_index, spritesheet) in spritesheets.iter().enumerate() {
        let spritesheet_image = DynamicImage::ImageRgba8(spritesheet.image.clone());

        let spritesheet_name = if spritesheets.len() == 1 {
            format!("{}.png", dir_name)
        } else {
            format!("{}_{}.png", dir_name, sheet_index + 1)
        };

        let mut spritesheet_data = Vec::new();
        spritesheet_image.write_to(
            &mut Cursor::new(&mut spritesheet_data),
            image::ImageFormat::Png,
        )?;

        let asset = Asset::new(
            spritesheet_name.clone(),
            spritesheet_data,
            "png",
            state.fontdb.clone(),
            false,
        )
        .await?;

        let spritesheet_path = format!("_spritesheets/{}", spritesheet_name);

        let needs_sync = match state.existing_lockfile.entries.get(&spritesheet_path) {
            Some(entry) => entry.hash.as_ref() != Some(&asset.hash()),
            None => true,
        };

        if !needs_sync {
            debug!("Skipping spritesheet {spritesheet_path} as it hasn't changed");
            continue;
        }

        if state.dry_run {
            if let Some(stats) = dry_run_stats {
                stats.pending_spritesheets.push(spritesheet_path.clone());
                stats.pending_sprite_count += spritesheet.sprites.len();
            }
            info!(
                "Would upload spritesheet: {} with {} sprites",
                spritesheet_path,
                spritesheet.sprites.len()
            );
            continue;
        }

        let sync_result = match &backend {
            TargetBackend::Cloud(backend) => backend.sync(state, &spritesheet_path, &asset).await,
            TargetBackend::Studio(backend) => backend.sync(state, &spritesheet_path, &asset).await,
            TargetBackend::Debug(backend) => backend.sync(state, &spritesheet_path, &asset).await,
        }?;

        match sync_result {
            SyncResult::Cloud(asset_id) => {
                info!(
                    "Uploaded spritesheet {} with ID {}",
                    spritesheet_path, asset_id
                );

                state.new_lockfile.entries.insert(
                    spritesheet_path.clone(),
                    FileEntry {
                        hash: Some(asset.hash()),
                        asset_id,
                        sprite: None,
                    },
                );

                let asset_id_str = format_asset_id(asset_id);

                for (path, info) in &spritesheet.sprites {
                    let sprite_asset = AssetValue::Sprite {
                        id: asset_id_str.clone(),
                        x: info.x,
                        y: info.y,
                        width: info.width,
                        height: info.height,
                    };

                    sprite_assets.insert(path.clone(), sprite_asset);

                    state.new_lockfile.entries.insert(
                        path.clone(),
                        FileEntry {
                            hash: None,
                            asset_id,
                            sprite: Some(SpriteInfo {
                                x: info.x,
                                y: info.y,
                                width: info.width,
                                height: info.height,
                                spritesheet: spritesheet_path.clone(),
                            }),
                        },
                    );
                }
            }
            SyncResult::Studio(asset_id) => {
                info!("Synced spritesheet {} to Studio", spritesheet_path);

                for (path, info) in &spritesheet.sprites {
                    let sprite_asset = AssetValue::Sprite {
                        id: asset_id.clone(),
                        x: info.x,
                        y: info.y,
                        width: info.width,
                        height: info.height,
                    };

                    sprite_assets.insert(path.clone(), sprite_asset);
                }
            }
            SyncResult::None => {
                info!("No result for spritesheet {}", spritesheet_path);
            }
        }
    }

    Ok(sprite_assets)
}

pub async fn sync(args: SyncArgs, existing_lockfile: LockFile) -> anyhow::Result<()> {
    let config = SyncConfig::read().await.context("Failed to read config")?;

    let mut state = SyncState::new(args, config, existing_lockfile)
        .await
        .context("Failed to create state")?;

    info!("Syncing...");

    let mut assets = BTreeMap::<String, AssetValue>::new();
    let mut synced = 0;

    let mut dry_run_stats = if state.dry_run {
        Some(DryRunStats::default())
    } else {
        None
    };

    let backend = match state.target {
        SyncTarget::Cloud => TargetBackend::Cloud(CloudBackend),
        SyncTarget::Studio => TargetBackend::Studio(StudioBackend::new().await?),
        SyncTarget::Debug => TargetBackend::Debug(DebugBackend::new().await?),
    };

    let sprite_assets = process_spritesheets(&mut state, &backend, &mut dry_run_stats).await?;
    assets.extend(sprite_assets);

    let mut sprite_paths = HashSet::new();
    for (_, sprites) in state.new_lockfile.entries.iter() {
        if let Some(sprite) = &sprites.sprite {
            sprite_paths.insert(sprite.spritesheet.clone());
        }
    }

    for entry in WalkDir::new(&state.asset_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let path_str = path.to_str().unwrap();
        let fixed_path = fix_path(path_str);

        if sprite_paths.contains(&fixed_path) || state.is_in_spritesheet(&fixed_path) {
            debug!("Skipping file that's part of a spritesheet: {}", fixed_path);
            continue;
        }

        if state.exclude_assets_matcher.is_match(path_str) {
            continue;
        }

        let result = match process_file(&entry, &mut state, &backend, &mut dry_run_stats).await {
            Ok(Some(result)) => {
                synced += 1;
                result
            }
            Ok(None) => {
                synced += 1;
                continue;
            }
            Err(e) => {
                warn!("Failed to process file {fixed_path}: {e:?}");
                continue;
            }
        };

        assets.insert(fixed_path.clone(), result.asset_value);
        if let Some(file_entry) = result.file_entry {
            state.new_lockfile.entries.insert(fixed_path, file_entry);
        }
    }

    if state.dry_run {
        if let Some(stats) = dry_run_stats {
            info!(
                "Dry run completed. {} asset{} would be synced.",
                synced,
                if synced == 1 { "" } else { "s" }
            );

            stats.report();

            if stats.has_pending_assets() {
                return Err(anyhow!(
                    "Assets need to be uploaded. Run without --dry-run to upload them."
                ));
            }
        }
        return Ok(());
    }

    if matches!(state.target, SyncTarget::Debug) {
        info!(
            "Synced {} asset{}!",
            synced,
            if synced == 1 { "" } else { "s" }
        );
        return Ok(());
    }

    if let SyncTarget::Cloud = state.target {
        state
            .new_lockfile
            .write(Path::new(crate::lockfile::FILE_NAME))
            .await
            .context("Failed to write lockfile")?;
    }

    let asset_dir = state.asset_dir.to_str().unwrap();

    // Fixed this section to maintain relative paths
    for (path, asset) in state.existing.iter() {
        // Paths in state.existing should already be relative to asset_dir
        // Use them directly without modification
        assets.insert(path.clone(), AssetValue::Asset(format_asset_id(asset.id)));
    }

    let luau_filename = format!("{}.{}", state.output_name, "luau");
    let luau_output = generate_luau(&assets, asset_dir, &state.style, state.strip_extension)?;

    fs::write(Path::new(&state.write_dir).join(luau_filename), luau_output)
        .await
        .context("Failed to write output Luau file")?;

    if state.typescript {
        let ts_filename = format!("{}.d.ts", state.output_name);
        let ts_output = generate_ts(
            &assets,
            asset_dir,
            state.output_name.as_str(),
            &state.style,
            state.strip_extension,
        )?;

        fs::write(Path::new(&state.write_dir).join(ts_filename), ts_output)
            .await
            .context("Failed to write output TypeScript file")?;
    }

    info!(
        "Synced {} asset{}!",
        synced,
        if synced == 1 { "" } else { "s" }
    );

    Ok(())
}
