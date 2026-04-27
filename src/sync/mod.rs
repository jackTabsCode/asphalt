use crate::{
    asset::{self, Asset, AssetRef},
    cli::{SyncArgs, SyncTarget},
    config::Config,
    hash::Hash,
    lockfile::{LockfileEntry, RawLockfile},
    sync::{backend::Backend, collect::collect_events},
};
use anyhow::{Context, bail};
use fs_err::tokio as fs;
use indicatif::MultiProgress;
use log::{info, warn};
use relative_path::{PathExt, RelativePathBuf};
use resvg::usvg::fontdb;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::mpsc::{self};
use walkdir::WalkDir;

mod backend;
mod codegen;
mod collect;
mod walk;

enum TargetBackend {
    Cloud(backend::Cloud),
    Debug(backend::Debug),
    Studio(backend::Studio),
}

impl TargetBackend {
    pub async fn sync(
        &self,
        asset: &Asset,
        lockfile_entry: Option<&LockfileEntry>,
    ) -> anyhow::Result<Option<AssetRef>> {
        match self {
            Self::Cloud(cloud_backend) => cloud_backend.sync(asset, lockfile_entry).await,
            Self::Debug(debug_backend) => debug_backend.sync(asset, lockfile_entry).await,
            Self::Studio(studio_backend) => studio_backend.sync(asset, lockfile_entry).await,
        }
    }
}

#[derive(Debug)]
enum Event {
    Discovered(PathBuf),
    InFlight(PathBuf),
    Finished {
        state: EventState,
        input_name: String,
        path: PathBuf,
        rel_path: RelativePathBuf,
        hash: Hash,
        asset_ref: Option<AssetRef>,
    },
    Failed(PathBuf),
}

#[derive(Debug)]
enum EventState {
    Synced { new: bool },
    Duplicate,
}

pub async fn sync(args: SyncArgs, mp: MultiProgress) -> anyhow::Result<()> {
    let config = Config::read_from(args.project.clone()).await?;
    let target = args.target();
    let watch = target.is_watch();

    let font_db = Arc::new({
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    });

    let backend = Arc::new({
        let params = backend::Params {
            api_key: args.api_key,
            creator: config.creator.clone(),
            expected_price: args.expected_price,
            project_dir: config.project_dir.clone(),
        };
        match &target {
            SyncTarget::Cloud { dry_run: false } => {
                Some(TargetBackend::Cloud(backend::Cloud::new(params).await?))
            }
            SyncTarget::Cloud { dry_run: true } => None,
            SyncTarget::Debug => Some(TargetBackend::Debug(backend::Debug::new(params).await?)),
            SyncTarget::Studio { .. } => {
                Some(TargetBackend::Studio(backend::Studio::new(params).await?))
            }
        }
    });

    // Run the initial sync through the standard pipeline
    run_initial_sync(&config, target, &backend, &font_db, mp).await?;

    // If watch mode, enter the studio polling loop
    if watch {
        let studio = match *backend {
            Some(TargetBackend::Studio(ref s)) => s,
            _ => bail!("--watch is only supported with studio target"),
        };
        studio_watch_loop(&config, studio, font_db).await?;
    }

    Ok(())
}

/// Runs one full sync cycle through the standard walk/collect pipeline.
async fn run_initial_sync(
    config: &Config,
    target: SyncTarget,
    backend: &Arc<Option<TargetBackend>>,
    font_db: &Arc<fontdb::Database>,
    mp: MultiProgress,
) -> anyhow::Result<()> {
    let existing_lockfile = RawLockfile::read_from(&config.project_dir)
        .await?
        .into_lockfile()?;

    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();

    let collector_handle = tokio::spawn({
        let inputs = config.inputs.clone();
        let project_dir = config.project_dir.clone();
        async move { collect_events(event_rx, target, inputs, mp, &project_dir).await }
    });

    let params = walk::Params {
        target,
        existing_lockfile,
        font_db: font_db.clone(),
        backend: backend.clone(),
    };

    walk::walk(params, config, &event_tx).await;
    drop(event_tx);

    let results = collector_handle.await??;

    if matches!(target, SyncTarget::Cloud { dry_run: true }) {
        if results.new_count > 0 {
            bail!("Dry run: {} new assets would be synced", results.new_count)
        } else {
            info!("Dry run: No new assets would be synced");
            return Ok(());
        }
    }

    if target.write_on_sync() {
        results.new_lockfile.write_to(&config.project_dir).await?;
    }

    for (input_name, source) in results.input_sources {
        let input = config
            .inputs
            .get(&input_name)
            .context("Failed to find input for codegen input")?;

        write_codegen(config, &input_name, &source, &input.output_path, true).await?;
    }

    if results.any_failed {
        bail!("Some assets failed to sync")
    }

    Ok(())
}

/// Write codegen output files, optionally skipping if content is unchanged.
async fn write_codegen(
    config: &Config,
    input_name: &str,
    source: &codegen::NodeSource,
    output_path: &Path,
    always_write: bool,
) -> anyhow::Result<()> {
    let mut langs = vec![codegen::Language::Luau];
    if config.codegen.typescript {
        langs.push(codegen::Language::TypeScript);
    }

    let abs_output = config.project_dir.join(output_path);
    fs::create_dir_all(&abs_output).await?;

    for lang in langs {
        let node = codegen::create_node(source, &config.codegen);
        let ext = match lang {
            codegen::Language::Luau => "luau",
            codegen::Language::TypeScript => "d.ts",
        };
        let code = codegen::generate_code(lang, input_name, &node)?;
        let file_path = abs_output.join(format!("{input_name}.{ext}"));

        if always_write {
            fs::write(&file_path, code).await?;
        } else {
            // Skip write if content is identical (avoid triggering downstream watchers)
            let existing = fs::read_to_string(&file_path).await.unwrap_or_default();
            if code != existing {
                fs::write(&file_path, code).await?;
            }
        }
    }

    Ok(())
}

// --- Studio watch mode (polling) ---

struct FileEntry {
    hash: Hash,
    input_name: String,
    rel_path: RelativePathBuf,
    ext: String,
}

type FileState = HashMap<PathBuf, FileEntry>;

/// Scan all input directories and compute hashes for every matching file.
/// Does NOT process assets (no SVG→PNG, no alpha bleed) — only reads + hashes.
async fn scan_all_inputs(config: &Config) -> anyhow::Result<FileState> {
    let mut state = FileState::new();

    for (input_name, input) in &config.inputs {
        let input_prefix = config.project_dir.join(input.include.get_prefix());

        for entry in WalkDir::new(&input_prefix)
            .into_iter()
            .filter_entry(|entry| {
                let path = entry.path();
                if path == input_prefix {
                    return true;
                }
                if let Ok(rel_path) = path.strip_prefix(&config.project_dir) {
                    input.include.is_match(rel_path)
                } else {
                    false
                }
            })
        {
            let Ok(entry) = entry else { continue };
            let path = entry.into_path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension() else {
                continue;
            };
            if !asset::is_supported_extension(ext) {
                continue;
            }
            let ext_str = ext.to_string_lossy().to_string();

            let rel_path = path.relative_to(&input_prefix)?;
            let data = fs::read(&path).await?;
            let asset =
                Asset::new(rel_path.clone(), data.into()).context("Failed to create asset")?;

            state.insert(
                path,
                FileEntry {
                    hash: asset.hash,
                    input_name: input_name.clone(),
                    rel_path,
                    ext: ext_str,
                },
            );
        }
    }

    Ok(state)
}

/// Studio watch polling loop. Runs after the initial sync completes.
async fn studio_watch_loop(
    initial_config: &Config,
    studio: &backend::Studio,
    font_db: Arc<fontdb::Database>,
) -> anyhow::Result<()> {
    let project_dir = initial_config.project_dir.clone();
    let mut prev_state = scan_all_inputs(initial_config).await?;

    info!("Watching for file changes... (press Ctrl+C to stop)");

    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Re-read config in case asphalt.toml changed
        let config = match Config::read_from(project_dir.clone()).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read config: {e:?}");
                continue;
            }
        };

        let current_state = match scan_all_inputs(&config).await {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to scan inputs: {e:?}");
                continue;
            }
        };

        // Diff: find added, removed, changed
        let mut affected_inputs: HashSet<String> = HashSet::new();
        let mut added_or_changed: Vec<PathBuf> = Vec::new();
        let mut any_removed = false;

        for (path, entry) in &current_state {
            match prev_state.get(path) {
                None => {
                    added_or_changed.push(path.clone());
                    affected_inputs.insert(entry.input_name.clone());
                }
                Some(prev_entry) if prev_entry.hash != entry.hash => {
                    added_or_changed.push(path.clone());
                    affected_inputs.insert(entry.input_name.clone());
                }
                _ => {}
            }
        }
        for (path, entry) in &prev_state {
            if !current_state.contains_key(path) {
                any_removed = true;
                affected_inputs.insert(entry.input_name.clone());
            }
        }

        if affected_inputs.is_empty() {
            continue;
        }

        info!(
            "File changes detected: {} added/changed{}",
            added_or_changed.len(),
            if any_removed { ", some removed" } else { "" }
        );

        // Process only changed/added files
        for path in &added_or_changed {
            let entry = &current_state[path];
            let data = fs::read(path).await?;
            let mut asset = Asset::new(entry.rel_path.clone(), data.into())
                .context("Failed to create asset")?;

            let bleed = config
                .inputs
                .get(&entry.input_name)
                .map(|i| i.bleed)
                .unwrap_or(true);

            let font_db = font_db.clone();
            asset = tokio::task::spawn_blocking(move || -> anyhow::Result<Asset> {
                let mut asset = asset;
                asset.process(font_db, bleed)?;
                Ok(asset)
            })
            .await?
            .context("Failed to process asset")?;

            studio.sync(&asset, None).await?;
        }

        // Clean orphans if files were removed
        if any_removed {
            let valid: HashSet<Hash> = current_state.values().map(|e| e.hash).collect();
            studio.clean_orphans(&valid).await?;
        }

        // Regenerate codegen only for affected inputs
        for input_name in &affected_inputs {
            let Some(input) = config.inputs.get(input_name) else {
                continue;
            };

            let mut source = codegen::NodeSource::new();
            for (rel_path, web) in &input.web {
                source.insert(rel_path.clone(), AssetRef::Cloud(web.id));
            }
            for entry in current_state.values() {
                if entry.input_name == *input_name {
                    source.insert(
                        entry.rel_path.clone(),
                        studio.ref_for_hash(&entry.hash, &entry.ext),
                    );
                }
            }

            write_codegen(&config, input_name, &source, &input.output_path, false).await?;
        }

        prev_state = current_state;
    }
}
