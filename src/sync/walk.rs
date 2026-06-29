use crate::{
    asset::{self, Asset, AssetRef},
    cli::{SyncArgs, SyncTarget},
    config::{Config, Input},
    hash::Hash,
    lockfile::{Lockfile, LockfileEntry, SpriteInfo},
    sync::{TargetBackend, atlas_node, pack_assets, should_pack},
};
use anyhow::Context;
use fs_err::tokio as fs;
use log::{debug, warn};
use relative_path::{PathExt, RelativePathBuf};
use resvg::usvg::fontdb;
use std::{
    collections::{
        HashMap,
        hash_map::{self},
    },
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    sync::{Mutex, Semaphore, mpsc::UnboundedSender},
    task::JoinSet,
};
use walkdir::WalkDir;

pub struct Params {
    pub args: SyncArgs,
    pub target: SyncTarget,
    pub existing_lockfile: Lockfile,
    pub font_db: Arc<fontdb::Database>,
    pub backend: Option<TargetBackend>,
}

struct InputState {
    params: Arc<Params>,
    input_name: String,
    input_prefix: PathBuf,
    seen_hashes: Arc<Mutex<HashMap<Hash, PathBuf>>>,
    bleed: bool,
    warn_each_duplicate: bool,
}

pub async fn walk(params: Params, config: &Config, tx: &UnboundedSender<super::Event>) {
    let params = Arc::new(params);

    for (input_name, input) in &config.inputs {
        if should_pack(input, &params.args) {
            if let Err(err) = walk_packed_input(params.clone(), config, input_name, input, tx).await
            {
                warn!("Failed to pack input {input_name}: {err:?}");
            }
        } else {
            walk_unpacked_input(params.clone(), config, input_name, input, tx).await;
        }
    }
}

async fn walk_unpacked_input(
    params: Arc<Params>,
    config: &Config,
    input_name: &str,
    input: &Input,
    tx: &UnboundedSender<super::Event>,
) {
    let input_prefix = config.project_dir.join(input.include.get_prefix());

    let state = Arc::new(InputState {
        params: params.clone(),
        input_name: input_name.to_string(),
        input_prefix: input_prefix.clone(),
        seen_hashes: Arc::new(Mutex::new(HashMap::new())),
        bleed: input.bleed,
        warn_each_duplicate: input.warn_each_duplicate,
    });

    let mut join_set = JoinSet::new();
    let semaphore = Arc::new(Semaphore::new(50));

    for path in input_paths(config, input) {
        let state = state.clone();
        let semaphore = semaphore.clone();
        let tx = tx.clone();

        tx.send(super::Event::Discovered(path.clone())).unwrap();

        join_set.spawn(async move {
            let _permit = semaphore.acquire_owned().await.unwrap();

            tx.send(super::Event::InFlight(path.clone())).unwrap();

            if let Err(err) = process_entry(state.clone(), &path, &tx).await {
                warn!("Failed to process file {}: {err:?}", path.display());
                tx.send(super::Event::Failed(path.clone())).unwrap();
            }
        });
    }

    while join_set.join_next().await.is_some() {}
}

async fn process_entry(
    state: Arc<InputState>,
    path: &Path,
    tx: &UnboundedSender<super::Event>,
) -> anyhow::Result<()> {
    debug!("Handling entry: {}", path.display());

    let rel_path = path.relative_to(&state.input_prefix)?;
    let data = fs::read(path).await?;
    let mut asset = Asset::new(rel_path.clone(), data.into()).context("Failed to create asset")?;

    let lockfile_entry = state
        .params
        .existing_lockfile
        .get(&state.input_name, &asset.hash);

    if send_duplicate_if_seen(&state, path, &rel_path, &asset, tx).await? {
        return Ok(());
    }

    let always_target = matches!(state.params.target, SyncTarget::Studio | SyncTarget::Debug);
    let is_new = always_target || lockfile_entry.is_none();

    if is_new {
        asset = process_asset(
            asset,
            state.params.font_db.clone(),
            state.bleed,
            state.params.args.optimize,
        )
        .await?;
    }

    let asset_ref = match state.params.backend {
        Some(ref backend) => backend.sync(&asset, lockfile_entry).await?,
        None => lockfile_entry.map(Into::into),
    };

    let sprite_info = lockfile_entry.and_then(|entry| entry.sprite_info.clone());

    tx.send(super::Event::Finished {
        state: super::EventState::Synced { new: is_new },
        input_name: state.input_name.clone(),
        path: path.into(),
        rel_path: asset.path.clone(),
        hash: asset.hash,
        asset_ref,
        node: sprite_info.as_ref().and_then(|info| {
            lockfile_entry.map(|entry| Box::new(node_from_sprite_info(entry.asset_id, info)))
        }),
        sprite_info,
    })
    .unwrap();

    Ok(())
}

async fn send_duplicate_if_seen(
    state: &Arc<InputState>,
    path: &Path,
    rel_path: &RelativePathBuf,
    asset: &Asset,
    tx: &UnboundedSender<super::Event>,
) -> anyhow::Result<bool> {
    let mut seen_hashes = state.seen_hashes.lock().await;

    match seen_hashes.entry(asset.hash) {
        hash_map::Entry::Occupied(entry) => {
            let seen_path = entry.get();
            let rel_seen_path = seen_path.relative_to(&state.input_prefix)?;

            debug!("Duplicate asset found: {} -> {}", rel_path, rel_seen_path);
            if state.warn_each_duplicate {
                warn!(
                    "Duplicate file found: {} (original at {})",
                    rel_path, rel_seen_path
                );
            }

            let lockfile_entry = state
                .params
                .existing_lockfile
                .get(&state.input_name, &asset.hash);

            tx.send(super::Event::Finished {
                state: super::EventState::Duplicate,
                input_name: state.input_name.clone(),
                path: path.into(),
                rel_path: rel_path.clone(),
                asset_ref: lockfile_entry.map(Into::into),
                hash: asset.hash,
                node: None,
                sprite_info: lockfile_entry.and_then(|entry| entry.sprite_info.clone()),
            })
            .unwrap();

            Ok(true)
        }
        hash_map::Entry::Vacant(_) => {
            seen_hashes.insert(asset.hash, path.into());
            Ok(false)
        }
    }
}

async fn walk_packed_input(
    params: Arc<Params>,
    config: &Config,
    input_name: &str,
    input: &Input,
    tx: &UnboundedSender<super::Event>,
) -> anyhow::Result<()> {
    let input_prefix = config.project_dir.join(input.include.get_prefix());
    let mut seen_hashes = HashMap::<Hash, PathBuf>::new();
    let mut image_assets = Vec::new();
    let mut passthrough_assets = Vec::new();
    let mut existing_images = Vec::new();
    let mut has_new_image = false;

    for path in input_paths(config, input) {
        tx.send(super::Event::Discovered(path.clone())).unwrap();
        tx.send(super::Event::InFlight(path.clone())).unwrap();

        match read_asset(&path, &input_prefix).await {
            Ok(asset) => {
                if let hash_map::Entry::Occupied(entry) = seen_hashes.entry(asset.hash) {
                    if input.warn_each_duplicate
                        && let Ok(original_path) = entry.get().relative_to(&input_prefix)
                    {
                        warn!(
                            "Duplicate file found: {} (original at {})",
                            asset.path, original_path
                        );
                    }
                    let lockfile_entry = params.existing_lockfile.get(input_name, &asset.hash);
                    tx.send(super::Event::Finished {
                        state: super::EventState::Duplicate,
                        input_name: input_name.to_string(),
                        path,
                        rel_path: asset.path,
                        hash: asset.hash,
                        asset_ref: lockfile_entry.map(Into::into),
                        node: None,
                        sprite_info: lockfile_entry.and_then(|entry| entry.sprite_info.clone()),
                    })
                    .unwrap();
                    continue;
                }
                seen_hashes.insert(asset.hash, path.clone());

                let lockfile_entry = params.existing_lockfile.get(input_name, &asset.hash);
                let is_image = matches!(asset.ty, crate::asset::AssetType::Image(_));
                let always_target = matches!(params.target, SyncTarget::Studio | SyncTarget::Debug);
                let is_new = always_target || lockfile_entry.is_none();

                if is_image {
                    has_new_image |= is_new;
                    if is_new {
                        image_assets.push(process_asset(
                            asset,
                            params.font_db.clone(),
                            input.bleed,
                            params.args.optimize,
                        ));
                    } else {
                        existing_images.push((path, asset, lockfile_entry.cloned()));
                    }
                } else if is_new {
                    passthrough_assets.push(process_asset(
                        asset,
                        params.font_db.clone(),
                        input.bleed,
                        params.args.optimize,
                    ));
                } else {
                    send_existing(input_name, path, asset, lockfile_entry, tx);
                }
            }
            Err(err) => {
                warn!("Failed to process file {}: {err:?}", path.display());
                tx.send(super::Event::Failed(path)).unwrap();
            }
        }
    }

    if !has_new_image && matches!(params.target, SyncTarget::Cloud { dry_run: false }) {
        for (path, asset, entry) in existing_images {
            send_existing(input_name, path, asset, entry.as_ref(), tx);
        }

        for asset in passthrough_assets {
            let asset = asset.await?;
            sync_single_asset(input_name, asset, &params, tx).await?;
        }

        return Ok(());
    }

    let mut assets = Vec::new();
    for asset in image_assets {
        assets.push(asset.await?);
    }
    for asset in passthrough_assets {
        assets.push(asset.await?);
    }
    for (_, asset, _) in existing_images {
        assets.push(
            process_asset(
                asset,
                params.font_db.clone(),
                input.bleed,
                params.args.optimize,
            )
            .await?,
        );
    }

    let (assets, metadata) = pack_assets(assets, input_name, input, &params.args)?;
    let metadata = metadata.as_ref();

    for asset in assets {
        let atlas_page_index = atlas_page_index(&asset.path);
        let lockfile_entry = params.existing_lockfile.get(input_name, &asset.hash);
        let asset_ref = match params.backend {
            Some(ref backend) => backend.sync(&asset, lockfile_entry).await?,
            None => lockfile_entry.map(Into::into),
        };

        if let (Some(page_index), Some(metadata), Some(asset_ref)) =
            (atlas_page_index, metadata, asset_ref.clone())
        {
            send_atlas_sprite_events(input_name, &asset, page_index, asset_ref, metadata, tx);
        } else {
            tx.send(super::Event::Finished {
                state: super::EventState::Synced {
                    new: lockfile_entry.is_none()
                        || matches!(params.target, SyncTarget::Studio | SyncTarget::Debug),
                },
                input_name: input_name.to_string(),
                path: asset.path.to_path(&input_prefix),
                rel_path: asset.path.clone(),
                hash: asset.hash,
                asset_ref,
                node: None,
                sprite_info: None,
            })
            .unwrap();
        }
    }

    Ok(())
}

async fn sync_single_asset(
    input_name: &str,
    asset: Asset,
    params: &Params,
    tx: &UnboundedSender<super::Event>,
) -> anyhow::Result<()> {
    let lockfile_entry = params.existing_lockfile.get(input_name, &asset.hash);
    let is_new =
        lockfile_entry.is_none() || matches!(params.target, SyncTarget::Studio | SyncTarget::Debug);
    let asset_ref = match params.backend {
        Some(ref backend) => backend.sync(&asset, lockfile_entry).await?,
        None => lockfile_entry.map(Into::into),
    };

    tx.send(super::Event::Finished {
        state: super::EventState::Synced { new: is_new },
        input_name: input_name.to_string(),
        path: asset.path.to_path(""),
        rel_path: asset.path.clone(),
        hash: asset.hash,
        asset_ref,
        node: None,
        sprite_info: None,
    })
    .unwrap();

    Ok(())
}

fn send_existing(
    input_name: &str,
    path: PathBuf,
    asset: Asset,
    lockfile_entry: Option<&LockfileEntry>,
    tx: &UnboundedSender<super::Event>,
) {
    let sprite_info = lockfile_entry.and_then(|entry| entry.sprite_info.clone());
    tx.send(super::Event::Finished {
        state: super::EventState::Synced { new: false },
        input_name: input_name.to_string(),
        path,
        rel_path: asset.path,
        hash: asset.hash,
        asset_ref: lockfile_entry.map(Into::into),
        node: sprite_info.as_ref().and_then(|info| {
            lockfile_entry.map(|entry| Box::new(node_from_sprite_info(entry.asset_id, info)))
        }),
        sprite_info,
    })
    .unwrap();
}

fn send_atlas_sprite_events(
    input_name: &str,
    atlas_asset: &Asset,
    page_index: usize,
    asset_ref: AssetRef,
    metadata: &super::PackingMetadata,
    tx: &UnboundedSender<super::Event>,
) {
    for (sprite_name, sprite_info) in &metadata.manifest.sprites {
        if sprite_info.page_index != page_index {
            continue;
        }

        let Some(rel_path) = metadata.sprite_to_path.get(sprite_name) else {
            continue;
        };
        let Some(hash) = metadata.sprite_to_hash.get(sprite_name) else {
            continue;
        };

        let lockfile_sprite_info = SpriteInfo {
            rect: sprite_info.rect,
            source_size: sprite_info.source_size,
            trimmed: sprite_info.trimmed,
            sprite_source_size: sprite_info.sprite_source_size,
        };

        tx.send(super::Event::Finished {
            state: super::EventState::Synced { new: true },
            input_name: input_name.to_string(),
            path: atlas_asset.path.to_path(""),
            rel_path: rel_path.clone(),
            hash: *hash,
            asset_ref: Some(asset_ref.clone()),
            node: Some(Box::new(atlas_node(asset_ref.to_string(), sprite_info))),
            sprite_info: Some(lockfile_sprite_info),
        })
        .unwrap();
    }
}

async fn process_asset(
    asset: Asset,
    font_db: Arc<fontdb::Database>,
    bleed: bool,
    optimize: bool,
) -> anyhow::Result<Asset> {
    tokio::task::spawn_blocking(move || -> anyhow::Result<Asset> {
        let mut asset = asset;
        asset.process(font_db, bleed, optimize)?;
        Ok(asset)
    })
    .await?
    .context("Failed to process asset")
}

async fn read_asset(path: &Path, input_prefix: &Path) -> anyhow::Result<Asset> {
    let rel_path = path.relative_to(input_prefix)?;
    let data = fs::read(path).await?;
    Asset::new(rel_path, data.into()).context("Failed to create asset")
}

fn input_paths(config: &Config, input: &Input) -> Vec<PathBuf> {
    let input_prefix = config.project_dir.join(input.include.get_prefix());

    WalkDir::new(&input_prefix)
        .into_iter()
        .filter_entry(|entry| {
            let path = entry.path();
            if path == input_prefix {
                return true;
            }
            if entry.file_type().is_dir() {
                return true;
            }
            if let Ok(rel_path) = path.strip_prefix(&config.project_dir) {
                input.include.is_match(rel_path)
            } else {
                false
            }
        })
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|path| path.is_file())
        .filter(|path| path.extension().is_some_and(asset::is_supported_extension))
        .collect()
}

fn atlas_page_index(path: &RelativePathBuf) -> Option<usize> {
    path.file_name()
        .and_then(|name| name.strip_suffix(".png"))
        .and_then(|name| name.rsplit_once("-sheet-"))
        .and_then(|(_, index)| index.parse().ok())
}

fn node_from_sprite_info(asset_id: u64, sprite_info: &SpriteInfo) -> super::codegen::Node {
    super::codegen::Node::AtlasSprite(super::codegen::AtlasSpriteData {
        image: format!("rbxassetid://{asset_id}"),
        rect: sprite_info.rect,
        size: sprite_info.source_size,
        trimmed: sprite_info.trimmed,
        sprite_source_size: sprite_info.sprite_source_size,
    })
}
