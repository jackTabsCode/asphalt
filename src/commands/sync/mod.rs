use self::state::SyncState;
use crate::{
    asset::Asset,
    cli::{SyncArgs, SyncTarget},
    lockfile::SpriteInfo,
    FileEntry, LockFile,
};
use anyhow::Context;
use backend::{
    cloud::CloudBackend, debug::DebugBackend, studio::StudioBackend, SyncBackend, SyncResult,
};
use codegen::{generate_luau, generate_ts, AssetValue};
use config::SyncConfig;
use image::DynamicImage;
use log::{debug, info, warn};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
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

async fn process_file(
    entry: &DirEntry,
    state: &mut SyncState,
    backend: &TargetBackend,
) -> anyhow::Result<Option<ProcessResult>> {
    let path = entry.path();
    let path_str = path.to_str().unwrap();
    let fixed_path = fix_path(path_str);

    debug!("Processing {fixed_path}");

    if state.is_in_spritesheet(&fixed_path) {
        debug!("Skipping {fixed_path} as it will be included in a spritesheet");
        return Ok(None);
    }

    let file_name = path
        .file_name()
        .with_context(|| format!("Failed to get file name of {}", fixed_path))?
        .to_str()
        .unwrap()
        .to_string();

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

    let asset = Asset::new(file_name, data, ext, state.fontdb.clone()).await?;
    let hash = asset.hash();

    if state.dry_run {
        info!("Sync {fixed_path}");
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
) -> anyhow::Result<(
    HashMap<String, (String, HashMap<String, SpriteInfo>)>,
    BTreeMap<String, AssetValue>,
)> {
    use crate::util::spritesheet;

    let asset_dir = PathBuf::from(&state.asset_dir);

    let packs = spritesheet::collect_images_for_packing(
        &asset_dir,
        &state.spritesheet_dirs,
        &state.exclude_assets_matcher,
        state.fontdb.clone(),
    )
    .await?;

    if packs.is_empty() {
        return Ok((HashMap::new(), BTreeMap::new()));
    }

    let mut results = HashMap::new();
    let mut sprite_assets = BTreeMap::new();

    for (pack_dir, images) in packs {
        if images.is_empty() {
            info!("No valid images found in directory: {}", pack_dir);
            continue;
        }

        let spritesheet = spritesheet::pack_spritesheet(&images)?;

        let spritesheet_image = DynamicImage::ImageRgba8(spritesheet.image.clone());

        let spritesheet_name = format!(
            "{}.png",
            Path::new(&pack_dir)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        );
        let mut spritesheet_data = Vec::new();
        spritesheet_image.write_to(
            &mut std::io::Cursor::new(&mut spritesheet_data),
            image::ImageFormat::Png,
        )?;

        let asset = Asset::new(
            spritesheet_name.clone(),
            spritesheet_data,
            "png",
            state.fontdb.clone(),
        )
        .await?;

        let spritesheet_path = format!("__SPRITESHEETS/{}", spritesheet_name);

        if !state.dry_run {
            let sync_result = match &backend {
                TargetBackend::Cloud(backend) => {
                    backend.sync(state, &spritesheet_path, &asset).await
                }
                TargetBackend::Studio(backend) => {
                    backend.sync(state, &spritesheet_path, &asset).await
                }
                TargetBackend::Debug(backend) => {
                    backend.sync(state, &spritesheet_path, &asset).await
                }
            }?;

            match sync_result {
                SyncResult::Cloud(asset_id) => {
                    state.new_lockfile.entries.insert(
                        spritesheet_path.clone(),
                        FileEntry {
                            hash: Some(asset.hash()),
                            asset_id,
                            sprite: None,
                        },
                    );

                    let asset_id_str = format_asset_id(asset_id);
                    let mut sprite_infos = HashMap::new();

                    for (path, info) in &spritesheet.sprites {
                        let sprite_info = SpriteInfo {
                            x: info.x,
                            y: info.y,
                            width: info.width,
                            height: info.height,
                            spritesheet: spritesheet_path.clone(),
                        };

                        sprite_infos.insert(path.clone(), sprite_info.clone());

                        state.new_lockfile.entries.insert(
                            path.clone(),
                            FileEntry {
                                hash: None,
                                asset_id,
                                sprite: Some(sprite_info),
                            },
                        );

                        let sprite_asset = AssetValue::Sprite {
                            id: asset_id_str.clone(),
                            x: info.x,
                            y: info.y,
                            width: info.width,
                            height: info.height,
                        };

                        sprite_assets.insert(path.clone(), sprite_asset);
                    }

                    results.insert(pack_dir, (asset_id_str, sprite_infos));
                }
                SyncResult::Studio(asset_id) => {
                    info!("Synced spritesheet {} to Studio", spritesheet_path);

                    let mut sprite_infos = HashMap::new();

                    for (path, info) in &spritesheet.sprites {
                        let sprite_info = SpriteInfo {
                            x: info.x,
                            y: info.y,
                            width: info.width,
                            height: info.height,
                            spritesheet: spritesheet_path.clone(),
                        };

                        sprite_infos.insert(path.clone(), sprite_info);

                        let sprite_asset = AssetValue::Sprite {
                            id: asset_id.clone(),
                            x: info.x,
                            y: info.y,
                            width: info.width,
                            height: info.height,
                        };

                        sprite_assets.insert(path.clone(), sprite_asset);
                    }

                    results.insert(pack_dir, (asset_id, sprite_infos));
                }
                SyncResult::None => {
                    info!("No result for spritesheet {}", spritesheet_path);
                }
            }
        } else {
            info!(
                "Would upload spritesheet: {} with {} sprites",
                spritesheet_path,
                spritesheet.sprites.len()
            );
        }
    }

    Ok((results, sprite_assets))
}

pub async fn sync(args: SyncArgs, existing_lockfile: LockFile) -> anyhow::Result<()> {
    let config = SyncConfig::read().await.context("Failed to read config")?;

    let mut state = SyncState::new(args, config, existing_lockfile)
        .await
        .context("Failed to create state")?;

    info!("Syncing...");

    let mut assets = BTreeMap::<String, AssetValue>::new();
    let mut synced = 0;

    let backend = match state.target {
        SyncTarget::Cloud => TargetBackend::Cloud(CloudBackend),
        SyncTarget::Studio => TargetBackend::Studio(StudioBackend::new().await?),
        SyncTarget::Debug => TargetBackend::Debug(DebugBackend::new().await?),
    };

    let (spritesheet_results, sprite_assets) = process_spritesheets(&mut state, &backend).await?;

    assets.extend(sprite_assets);

    let mut sprite_paths = HashSet::new();
    for (_, sprites) in spritesheet_results.values() {
        for path in sprites.keys() {
            sprite_paths.insert(path.clone());
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

        let result = match process_file(&entry, &mut state, &backend).await {
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

    if state.dry_run || matches!(state.target, SyncTarget::Debug) {
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

    for (path, asset) in state.existing.iter() {
        let mut path_buf = PathBuf::from(path);
        if !path_buf.starts_with(asset_dir) {
            path_buf = PathBuf::from(asset_dir).join(path);
        }
        let path_str = path_buf.to_str().unwrap().to_string();
        assets.insert(path_str, AssetValue::Asset(format_asset_id(asset.id)));
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
