use crate::{
    asset::Asset,
    auth::Auth,
    cli::{SyncArgs, SyncTarget},
    config::{Config, Input, PackOptions},
    lockfile::{Lockfile, LockfileEntry, RawLockfile},
    pack::Packer,
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

    let result_handle = {
        let codegen_tx = codegen_tx.clone();
        let lockfile_tx = lockfile_tx.clone();

        tokio::spawn(async move { handle_sync_results(result_rx, codegen_tx, lockfile_tx).await })
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

                    codegen_tx
                        .send(CodegenInsertion {
                            input_name: input_name.clone(),
                            asset_path: existing.path.clone(),
                            asset_id: format!("rbxassetid://{}", existing.entry.asset_id),
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
                            original_path,
                            path,
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
            } else {
                info!("No new assets would be synced.");
                return Ok(());
            }
        }

        let processed_assets =
            process::process(new_assets, state.clone(), input_name.clone(), input.bleed).await?;

        // Handle packing if enabled
        let final_assets = if should_pack(input, &args) {
            handle_packing(processed_assets, state.clone(), input_name.clone(), input, &args).await?
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

            fs::create_dir_all(&input.output_path).await?;
            fs::write(input.output_path.join(format!("{input_name}.{ext}")), code).await?;
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
) -> anyhow::Result<()> {
    while let Some(result) = rx.recv().await {
        if let BackendSyncResult::Cloud(asset_id) = result.backend {
            lockfile_tx
                .send(LockfileInsertion {
                    input_name: result.input_name.clone(),
                    hash: result.hash,
                    entry: LockfileEntry { asset_id },
                    write: true,
                })
                .await?;

            codegen_tx
                .send(CodegenInsertion {
                    input_name: result.input_name,
                    asset_path: result.path,
                    asset_id: format!("rbxassetid://{asset_id}"),
                })
                .await?;
        } else if let BackendSyncResult::Studio(asset_id) = result.backend {
            codegen_tx
                .send(CodegenInsertion {
                    input_name: result.input_name,
                    asset_path: result.path.clone(),
                    asset_id,
                })
                .await?;
        }
    }

    Ok(())
}

struct CodegenInsertion {
    input_name: String,
    asset_path: PathBuf,
    asset_id: String,
}

async fn collect_codegen_insertions(
    mut rx: Receiver<CodegenInsertion>,
    inputs: HashMap<String, Input>,
) -> anyhow::Result<HashMap<String, BTreeMap<PathBuf, String>>> {
    let mut inputs_to_sources: HashMap<String, BTreeMap<PathBuf, String>> = HashMap::new();

    for (input_name, input) in &inputs {
        for (path, asset) in &input.web {
            let entry = inputs_to_sources.entry(input_name.clone()).or_default();
            let path = PathBuf::from(path.replace('\\', "/"));

            entry.insert(path, format!("rbxassetid://{}", asset.id));
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

        source.insert(path.into(), insertion.asset_id);
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
    input.pack.as_ref().map_or(false, |pack| pack.enabled)
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

/// Handle packing of assets into atlases
async fn handle_packing(
    assets: Vec<Asset>,
    _state: Arc<SyncState>,
    input_name: String,
    input: &Input,
    args: &SyncArgs,
) -> anyhow::Result<Vec<Asset>> {

    let pack_options = apply_pack_overrides(input.pack.as_ref(), args);
    let packer = Packer::new(pack_options);

    // Filter only image assets for packing
    let (packable_assets, non_packable_assets): (Vec<_>, Vec<_>) = assets
        .into_iter()
        .partition(|asset| matches!(asset.ty, crate::asset::AssetType::Image(_)));

    if packable_assets.is_empty() {
        info!("No packable image assets found in input '{}'", input_name);
        return Ok(non_packable_assets);
    }

    info!("Packing {} images for input '{}'", packable_assets.len(), input_name);

    let pack_result = packer.pack_assets(&packable_assets, &input_name)?;

    if pack_result.atlases.is_empty() {
        warn!("No atlases were generated for input '{}'", input_name);
        return Ok(non_packable_assets);
    }

    let mut result_assets = non_packable_assets;
    let atlas_count = pack_result.atlases.len();

    // Convert atlases to assets
    for atlas in pack_result.atlases {
        let filename = format!("{}-sheet-{}.png", input_name, atlas.page_index);
        let atlas_path = input.output_path.join(&filename);

        // Write atlas PNG to disk
        fs::write(&atlas_path, &atlas.image_data).await?;

        // Create asset from atlas with a synthetic input path for sync compatibility
        let sync_path = input.path.get_prefix().join(&filename);
        let atlas_asset = Asset::new(sync_path, atlas.image_data)?;
        result_assets.push(atlas_asset);
    }

    // Write manifest JSON
    let manifest_filename = format!("{}.atlas.json", input_name);
    let manifest_path = input.output_path.join(&manifest_filename);
    let manifest_json = pack_result.manifest.to_json()?;
    fs::write(&manifest_path, manifest_json).await?;

    // Write Luau and TypeScript codegen files
    let luau_filename = format!("{}.atlas.luau", input_name);
    let luau_path = input.output_path.join(&luau_filename);
    let luau_code = pack_result.manifest.generate_luau(None);
    fs::write(&luau_path, luau_code).await?;

    let ts_filename = format!("{}.atlas.d.ts", input_name);
    let ts_path = input.output_path.join(&ts_filename);
    let ts_code = pack_result.manifest.generate_typescript();
    fs::write(&ts_path, ts_code).await?;

    info!(
        "Generated {} atlas pages and metadata for input '{}'",
        atlas_count,
        input_name
    );

    Ok(result_assets)
}
