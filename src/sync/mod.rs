use crate::{
    asset::Asset,
    auth::Auth,
    cli::{SyncArgs, SyncTarget},
    config::{Config, Input, PackOptions},
    lockfile::{Lockfile, LockfileEntry, RawLockfile},
    pack::{self, Packer},
    web_api::WebApiClient,
};
use anyhow::{Context, Result, bail};
use backend::BackendSyncResult;
use indicatif::MultiProgress;
use log::{info, warn};
use resvg::usvg::fontdb;
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};
use tokio::{
    fs,
    sync::mpsc::{self, Receiver, Sender},
};
use walk::{DuplicateResult, WalkResult};

mod backend;
mod codegen;
mod perform;
mod process;
mod walk;

pub struct SyncState {
    args: SyncArgs,

    existing_lockfile: Lockfile,
    result_tx: mpsc::Sender<SyncResult>,

    multi_progress: MultiProgress,

    font_db: Arc<fontdb::Database>,

    client: WebApiClient,
}

pub async fn sync(multi_progress: MultiProgress, args: SyncArgs) -> Result<()> {
    if args.dry_run && !matches!(args.target, SyncTarget::Cloud) {
        bail!("A dry run doesn't make sense in this context");
    }

    let config = Config::read().await?;
    let codegen_config = config.codegen.clone();

    let lockfile = RawLockfile::read().await?.into_lockfile()?;

    let key_required = matches!(args.target, SyncTarget::Cloud) && !args.dry_run;
    let auth = Auth::new(args.api_key.clone(), key_required)?;

    let font_db = Arc::new({
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    });

    let (codegen_tx, codegen_rx) = mpsc::channel::<CodegenInsertion>(100);

    let codegen_handle = {
        let inputs = config.inputs.clone();
        tokio::spawn(async move { collect_codegen_insertions(codegen_rx, inputs).await })
    };

    let (lockfile_tx, lockfile_rx) = mpsc::channel::<LockfileInsertion>(100);

    let lockfile_handle =
        tokio::spawn(async move { collect_lockfile_insertions(lockfile_rx).await });

    let (result_tx, result_rx) = mpsc::channel::<SyncResult>(100);

    let packing_metadata = Arc::new(tokio::sync::Mutex::new(
        HashMap::<String, PackingMetadata>::new(),
    ));

    let result_handle = {
        let codegen_tx = codegen_tx.clone();
        let lockfile_tx = lockfile_tx.clone();
        let packing_metadata = packing_metadata.clone();

        tokio::spawn(async move {
            handle_sync_results(result_rx, codegen_tx, lockfile_tx, packing_metadata).await
        })
    };

    let state = Arc::new(SyncState {
        args: args.clone(),

        existing_lockfile: lockfile,
        result_tx,

        multi_progress,

        font_db,

        client: WebApiClient::new(auth, config.creator, args.expected_price),
    });

    let mut duplicate_assets = HashMap::<String, Vec<DuplicateResult>>::new();

    for (input_name, input) in &config.inputs {
        let walk_results = walk::walk(state.clone(), input_name.clone(), input).await?;

        let mut new_assets = Vec::with_capacity(walk_results.len());
        let mut dupe_count = 0;

        for result in walk_results {
            match result {
                WalkResult::New(asset) => {
                    new_assets.push(asset);
                }
                WalkResult::Existing(existing) => {
                    if args.dry_run {
                        continue;
                    }

                    if matches!(args.target, SyncTarget::Cloud) {
                        lockfile_tx
                            .send(LockfileInsertion {
                                input_name: input_name.clone(),
                                hash: existing.hash,
                                entry: existing.entry.clone(),
                                // This takes too long, and we're not really losing anything here.
                                write: false,
                            })
                            .await?;
                    }

                    let node = if let Some(ref sprite_info) = existing.entry.sprite_info {
                        codegen::Node::AtlasSprite(codegen::AtlasSpriteData {
                            image: format!("rbxassetid://{}", existing.entry.asset_id),
                            rect: sprite_info.rect,
                            size: sprite_info.source_size,
                            trimmed: sprite_info.trimmed,
                            sprite_source_size: sprite_info.sprite_source_size,
                        })
                    } else {
                        codegen::Node::String(format!("rbxassetid://{}", existing.entry.asset_id))
                    };

                    codegen_tx
                        .send(CodegenInsertion {
                            input_name: input_name.clone(),
                            asset_path: existing.path.clone(),
                            node,
                        })
                        .await?;
                }
                WalkResult::Duplicate(dupe) => {
                    if input.warn_each_duplicate {
                        warn!(
                            "Duplicate file found: {} (original at {})",
                            dupe.path.display(),
                            dupe.original_path.display()
                        );
                    }

                    if args.dry_run {
                        continue;
                    }

                    dupe_count += 1;

                    let original_path = dupe
                        .original_path
                        .strip_prefix(input.path.get_prefix())
                        .unwrap()
                        .to_owned();

                    let path = dupe
                        .path
                        .strip_prefix(input.path.get_prefix())
                        .unwrap()
                        .to_owned();

                    duplicate_assets
                        .entry(input_name.clone())
                        .or_default()
                        .push(DuplicateResult {
                            path,
                            original_path,
                        });
                }
            }
        }

        if dupe_count > 0 {
            warn!("{dupe_count} duplicate files found.");
        }

        if args.dry_run {
            let new_len = new_assets.len();

            if new_len > 0 {
                bail!("{new_len} new assets would be synced!")
            }
            info!("No new assets would be synced.");
            return Ok(());
        }

        let processed_assets =
            process::process(new_assets, state.clone(), input_name.clone(), input.bleed).await?;

        // Handle packing if enabled
        let final_assets = if should_pack(input, &args) {
            let (assets, metadata) = handle_packing(
                processed_assets,
                state.clone(),
                input_name.clone(),
                input,
                &args,
            )
            .await?;

            if let Some(metadata) = metadata {
                packing_metadata
                    .lock()
                    .await
                    .insert(input_name.clone(), metadata);
            }

            assets
        } else {
            processed_assets
        };

        perform::perform(&final_assets, state.clone(), input_name.clone(), input).await?;
    }

    drop(state);

    result_handle.await??;

    drop(codegen_tx);
    drop(lockfile_tx);

    let new_lockfile = lockfile_handle.await??;
    if matches!(args.target, SyncTarget::Cloud) {
        new_lockfile.write(None).await?;
    }

    let mut inputs_to_sources = codegen_handle.await??;

    for (input_name, dupes) in duplicate_assets {
        let source = inputs_to_sources.get_mut(&input_name).unwrap();

        for dupe in dupes {
            let original = source.get(&dupe.original_path).unwrap();

            let path = dupe.path.to_string_lossy().replace('\\', "/");
            source.insert(path.into(), original.clone());
        }
    }

    for (input_name, source) in inputs_to_sources {
        let input = config
            .inputs
            .get(&input_name)
            .context("Failed to find input for codegen input")?;

        let mut langs_to_generate = vec![codegen::Language::Luau];

        if codegen_config.typescript {
            langs_to_generate.push(codegen::Language::TypeScript);
        }

        for lang in langs_to_generate {
            let node = codegen::create_node(&source, &config.codegen);
            let ext = match lang {
                codegen::Language::Luau => "luau",
                codegen::Language::TypeScript => "d.ts",
            };
            let code = codegen::generate_code(lang, &input_name, &node)?;

            fs::create_dir_all(&input.output_path)
                .await
                .with_context(|| {
                    format!(
                        "Failed to create output directory: {}",
                        input.output_path.display()
                    )
                })?;
            let output_file = input.output_path.join(format!("{input_name}.{ext}"));
            fs::write(&output_file, code).await.with_context(|| {
                format!("Failed to write codegen file: {}", output_file.display())
            })?;
        }
    }

    Ok(())
}

pub struct SyncResult {
    hash: String,
    path: PathBuf,
    input_name: String,
    backend: BackendSyncResult,
}

async fn handle_sync_results(
    mut rx: Receiver<SyncResult>,
    codegen_tx: Sender<CodegenInsertion>,
    lockfile_tx: Sender<LockfileInsertion>,
    packing_metadata: Arc<tokio::sync::Mutex<HashMap<String, PackingMetadata>>>,
) -> anyhow::Result<()> {
    while let Some(result) = rx.recv().await {
        // Check if this is an atlas upload
        let is_atlas = result.path.extension().is_some_and(|ext| ext == "png")
            && result
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| name.contains("-sheet-"));

        if let BackendSyncResult::Cloud(asset_id) = result.backend {
            if is_atlas {
                // Handle atlas upload - create AtlasSprite codegen entries
                handle_atlas_upload(
                    &result,
                    format!("rbxassetid://{asset_id}"),
                    &codegen_tx,
                    &lockfile_tx,
                    &packing_metadata,
                )
                .await?;
            } else {
                // Regular asset upload
                lockfile_tx
                    .send(LockfileInsertion {
                        input_name: result.input_name.clone(),
                        hash: result.hash,
                        entry: LockfileEntry {
                            asset_id,
                            sprite_info: None,
                        },
                        write: true,
                    })
                    .await?;

                codegen_tx
                    .send(CodegenInsertion {
                        input_name: result.input_name,
                        asset_path: result.path,
                        node: codegen::Node::String(format!("rbxassetid://{asset_id}")),
                    })
                    .await?;
            }
        } else if let BackendSyncResult::Studio(ref asset_id) = result.backend {
            if is_atlas {
                // Handle atlas upload for studio
                handle_atlas_upload(
                    &result,
                    asset_id.clone(),
                    &codegen_tx,
                    &lockfile_tx,
                    &packing_metadata,
                )
                .await?;
            } else {
                codegen_tx
                    .send(CodegenInsertion {
                        input_name: result.input_name,
                        asset_path: result.path.clone(),
                        node: codegen::Node::String(asset_id.clone()),
                    })
                    .await?;
            }
        }
    }

    Ok(())
}

async fn handle_atlas_upload(
    result: &SyncResult,
    atlas_asset_url: String,
    codegen_tx: &Sender<CodegenInsertion>,
    lockfile_tx: &Sender<LockfileInsertion>,
    packing_metadata: &Arc<tokio::sync::Mutex<HashMap<String, PackingMetadata>>>,
) -> anyhow::Result<()> {
    let metadata_guard = packing_metadata.lock().await;
    let Some(metadata) = metadata_guard.get(&result.input_name) else {
        let input = &result.input_name;
        log::warn!("No packing metadata found for input '{input}'");
        return Ok(());
    };

    // Extract page index from filename (format: "{input}-sheet-{N}.png")
    let filename = result
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid atlas filename")?;

    let page_index = filename
        .strip_suffix(".png")
        .and_then(|s| s.rsplit_once("-sheet-"))
        .and_then(|(_, idx)| idx.parse::<usize>().ok())
        .context("Failed to extract page index from atlas filename")?;

    // Find all sprites on this page and create AtlasSprite codegen entries
    for (sprite_name, sprite_info) in &metadata.manifest.sprites {
        if sprite_info.page_index != page_index {
            continue;
        }

        let Some(original_path) = metadata.sprite_to_path.get(sprite_name).cloned() else {
            log::warn!("No original path found for sprite '{sprite_name}'");
            continue;
        };

        codegen_tx
            .send(CodegenInsertion {
                input_name: result.input_name.clone(),
                asset_path: original_path,
                node: codegen::Node::AtlasSprite(codegen::AtlasSpriteData {
                    image: atlas_asset_url.clone(),
                    rect: sprite_info.rect,
                    size: sprite_info.source_size,
                    trimmed: sprite_info.trimmed,
                    sprite_source_size: sprite_info.sprite_source_size,
                }),
            })
            .await?;

        // Create lockfile entry for Cloud backend only
        if let BackendSyncResult::Cloud(asset_id) = result.backend {
            let Some(sprite_hash) = metadata.sprite_to_hash.get(sprite_name).cloned() else {
                log::warn!("No hash found for sprite '{sprite_name}'");
                continue;
            };

            let lockfile_sprite_info = crate::lockfile::SpriteInfo {
                rect: sprite_info.rect,
                source_size: sprite_info.source_size,
                trimmed: sprite_info.trimmed,
                sprite_source_size: sprite_info.sprite_source_size,
            };

            lockfile_tx
                .send(LockfileInsertion {
                    input_name: result.input_name.clone(),
                    hash: sprite_hash,
                    entry: LockfileEntry {
                        asset_id,
                        sprite_info: Some(lockfile_sprite_info),
                    },
                    write: true,
                })
                .await?;
        }
    }

    Ok(())
}

struct CodegenInsertion {
    input_name: String,
    asset_path: PathBuf,
    node: codegen::Node,
}

async fn collect_codegen_insertions(
    mut rx: Receiver<CodegenInsertion>,
    inputs: HashMap<String, Input>,
) -> anyhow::Result<HashMap<String, BTreeMap<PathBuf, codegen::Node>>> {
    let mut inputs_to_sources: HashMap<String, BTreeMap<PathBuf, codegen::Node>> = HashMap::new();

    for (input_name, input) in &inputs {
        for (path, asset) in &input.web {
            let entry = inputs_to_sources.entry(input_name.clone()).or_default();
            let path = PathBuf::from(path.replace('\\', "/"));

            entry.insert(
                path,
                codegen::Node::String(format!("rbxassetid://{}", asset.id)),
            );
        }
    }

    while let Some(insertion) = rx.recv().await {
        let source = inputs_to_sources
            .entry(insertion.input_name.clone())
            .or_default();

        let input = inputs
            .get(&insertion.input_name)
            .context("Failed to find input for codegen input")?;

        let path = insertion
            .asset_path
            .strip_prefix(input.path.get_prefix())
            .unwrap();

        let path = path.to_string_lossy().replace('\\', "/");

        source.insert(path.into(), insertion.node);
    }

    Ok(inputs_to_sources)
}

struct LockfileInsertion {
    input_name: String,
    hash: String,
    entry: LockfileEntry,
    write: bool,
}

async fn collect_lockfile_insertions(
    mut rx: Receiver<LockfileInsertion>,
) -> anyhow::Result<Lockfile> {
    let mut new_lockfile = Lockfile::default();

    while let Some(insertion) = rx.recv().await {
        new_lockfile.insert(&insertion.input_name, &insertion.hash, insertion.entry);
        if insertion.write {
            new_lockfile.write(None).await?;
        }
    }

    Ok(new_lockfile)
}

/// Check if packing should be enabled for this input
fn should_pack(input: &Input, args: &SyncArgs) -> bool {
    // CLI overrides
    if args.pack {
        return true;
    }
    if args.no_pack {
        return false;
    }

    // Check input configuration
    input.pack.as_ref().is_some_and(|pack| pack.enabled)
}

/// Apply CLI argument overrides to pack options
fn apply_pack_overrides(base_options: Option<&PackOptions>, args: &SyncArgs) -> PackOptions {
    let mut options = base_options.cloned().unwrap_or_default();

    // Apply CLI overrides
    if args.pack {
        options.enabled = true;
    }
    if args.no_pack {
        options.enabled = false;
    }
    if let Some(max_size) = args.pack_max_size {
        options.max_size = max_size;
    }
    if let Some(padding) = args.pack_padding {
        options.padding = padding;
    }
    if let Some(extrude) = args.pack_extrude {
        options.extrude = extrude;
    }
    if let Some(algorithm) = args.pack_algorithm.clone() {
        options.algorithm = algorithm;
    }
    if args.pack_trim {
        options.allow_trim = true;
    }
    if args.pack_no_trim {
        options.allow_trim = false;
    }
    if let Some(page_limit) = args.pack_page_limit {
        options.page_limit = Some(page_limit);
    }
    if let Some(sort) = args.pack_sort.clone() {
        options.sort = sort;
    }
    if args.pack_dedupe {
        options.dedupe = true;
    }

    options
}

struct PackingMetadata {
    manifest: pack::manifest::AtlasManifest,
    sprite_to_path: HashMap<String, PathBuf>,
    sprite_to_hash: HashMap<String, String>,
}

/// Handle packing of assets into atlases
async fn handle_packing(
    assets: Vec<Asset>,
    _state: Arc<SyncState>,
    input_name: String,
    input: &Input,
    args: &SyncArgs,
) -> anyhow::Result<(Vec<Asset>, Option<PackingMetadata>)> {
    let pack_options = apply_pack_overrides(input.pack.as_ref(), args);
    let packer = Packer::new(pack_options);

    // Filter only image assets for packing
    let (packable_assets, non_packable_assets): (Vec<_>, Vec<_>) = assets
        .into_iter()
        .partition(|asset| matches!(asset.ty, crate::asset::AssetType::Image(_)));

    if packable_assets.is_empty() {
        info!("No packable image assets found in input '{input_name}'");
        return Ok((non_packable_assets, None));
    }

    info!(
        "Packing {} images for input '{}'",
        packable_assets.len(),
        input_name
    );

    // Build sprite name to original path and hash mapping
    let mut sprite_to_path = HashMap::new();
    let mut sprite_to_hash = HashMap::new();
    for asset in &packable_assets {
        if let Some(name) = asset.path.file_stem().and_then(|s| s.to_str()) {
            sprite_to_path.insert(name.to_string(), asset.path.clone());
            sprite_to_hash.insert(name.to_string(), asset.hash.clone());
        }
    }

    let pack_result = packer.pack_assets(&packable_assets, &input_name)?;

    if pack_result.atlases.is_empty() {
        warn!("No atlases were generated for input '{input_name}'");
        return Ok((non_packable_assets, None));
    }

    let mut result_assets = non_packable_assets;
    let atlas_count = pack_result.atlases.len();

    // Convert atlases to assets (keep in memory, will be uploaded by backend)
    for atlas in &pack_result.atlases {
        let filename = format!("{}-sheet-{}.png", input_name, atlas.page_index);
        let sync_path = input.path.get_prefix().join(&filename);
        let atlas_asset = Asset::new(sync_path, atlas.image_data.clone())?;
        result_assets.push(atlas_asset);
    }

    info!("Generated {atlas_count} atlas pages for input '{input_name}'");

    let metadata = PackingMetadata {
        manifest: pack_result.manifest,
        sprite_to_path,
        sprite_to_hash,
    };

    Ok((result_assets, Some(metadata)))
}
