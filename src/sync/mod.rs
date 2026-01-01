use crate::{
    asset::{self, Asset, AssetRef},
    cli::{SyncArgs, SyncTarget},
    config::Config,
    lockfile::{Lockfile, LockfileEntry, RawLockfile},
    sync::{backend::Backend, codegen::NodeSource},
};
use anyhow::{Context, bail};
use dashmap::DashMap;
use futures::{StreamExt, stream};
use indicatif::MultiProgress;
use log::{debug, info, warn};
use relative_path::{PathExt, RelativePathBuf};
use resvg::usvg::fontdb;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{
    fs,
    sync::mpsc::{self, Receiver, Sender},
};
use walkdir::{DirEntry, WalkDir};

mod backend;
mod codegen;

pub struct State {
    args: SyncArgs,
    existing_lockfile: Lockfile,
    event_tx: Sender<Event>,
    multi_progress: MultiProgress,
    font_db: Arc<fontdb::Database>,
    target_backend: TargetBackend,
}

enum TargetBackend {
    Cloud(backend::Cloud),
    Debug(backend::Debug),
    Studio(backend::Studio),
}

impl TargetBackend {
    pub async fn sync(
        &self,
        state: Arc<State>,
        input_name: String,
        asset: &Asset,
    ) -> anyhow::Result<Option<AssetRef>> {
        match self {
            Self::Cloud(cloud_backend) => cloud_backend.sync(state, input_name, asset).await,
            Self::Debug(debug_backend) => debug_backend.sync(state, input_name, asset).await,
            Self::Studio(studio_backend) => studio_backend.sync(state, input_name, asset).await,
        }
    }
}

#[derive(Debug)]
enum Event {
    Insert {
        new: bool,
        input_name: String,
        path: RelativePathBuf,
        hash: String,
        asset_ref: AssetRef,
    },
    Duplicate {
        input_name: String,
        path: RelativePathBuf,
        original_path: RelativePathBuf,
    },
}

pub async fn sync(multi_progress: MultiProgress, args: SyncArgs) -> anyhow::Result<()> {
    if args.dry_run && !matches!(args.target, SyncTarget::Cloud) {
        bail!("A dry run doesn't make sense in this context");
    }

    let config = Config::read().await?;

    let existing_lockfile = RawLockfile::read().await?.into_lockfile()?;

    let font_db = Arc::new({
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    });

    let (event_tx, event_rx) = mpsc::channel::<Event>(100);

    let collector_handle = tokio::spawn({
        let config = config.clone();
        async move { collect_events(event_rx, config, args.dry_run).await }
    });

    walk(
        State {
            args: args.clone(),
            existing_lockfile,
            event_tx,
            multi_progress,
            font_db,
            target_backend: {
                let params = backend::Params {
                    api_key: args.api_key,
                    creator: config.creator.clone(),
                    expected_price: args.expected_price,
                };
                match args.target {
                    SyncTarget::Cloud => TargetBackend::Cloud(backend::Cloud::new(params).await?),
                    SyncTarget::Debug => TargetBackend::Debug(backend::Debug::new(params).await?),
                    SyncTarget::Studio => {
                        TargetBackend::Studio(backend::Studio::new(params).await?)
                    }
                }
            },
        },
        &config,
    )
    .await;

    let results = collector_handle.await??;

    if results.dupe_count > 0 {
        warn!("{} duplicate files found", results.dupe_count);
    }

    if args.dry_run {
        if results.new_count > 0 {
            bail!("Dry run: {} new assets would be synced", results.new_count)
        } else {
            info!("Dry run: No new assets would be synced");
            return Ok(());
        }
    }

    if matches!(args.target, SyncTarget::Cloud) {
        results.new_lockfile.write(None).await?;
    }

    for (input_name, source) in results.input_sources {
        let input = config
            .inputs
            .get(&input_name)
            .context("Failed to find input for codegen input")?;

        let mut langs_to_generate = vec![codegen::Language::Luau];

        if config.codegen.typescript {
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

struct InputState {
    sync_state: Arc<State>,
    input_name: String,
    input_prefix: PathBuf,
    seen_hashes: Arc<DashMap<String, PathBuf>>,
    bleed: bool,
}

async fn walk(state: State, config: &Config) {
    let state = Arc::new(state);

    for (input_name, input) in &config.inputs {
        let prefix = input.include.get_prefix();
        let entries = WalkDir::new(&prefix)
            .into_iter()
            .filter_entry(|entry| prefix == entry.path() || input.include.is_match(entry.path()))
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry
                        .path()
                        .extension()
                        .is_some_and(asset::is_supported_extension)
            })
            .collect::<Vec<_>>();

        let ctx = Arc::new(InputState {
            sync_state: state.clone(),
            input_name: input_name.clone(),
            input_prefix: prefix,
            seen_hashes: Arc::new(DashMap::with_capacity(entries.len())),
            bleed: input.bleed,
        });

        stream::iter(entries.iter())
            .for_each_concurrent(None, |entry| {
                eprintln!("Processing entry: {}", entry.path().display());
                let ctx = ctx.clone();
                async move {
                    if let Err(e) = handle_entry(ctx, entry).await {
                        debug!("Skipping file {}: {e:?}", entry.path().display());
                    }
                }
            })
            .await;
    }
}

async fn handle_entry(state: Arc<InputState>, entry: &DirEntry) -> anyhow::Result<()> {
    debug!("Handling entry: {}", entry.path().display());

    let data = fs::read(entry.path()).await?;
    let rel_path = entry.path().relative_to(&state.input_prefix)?;

    let mut asset = Asset::new(rel_path.clone(), data)
        .await
        .context("Failed to create asset")?;

    if let Some(seen_path) = state.seen_hashes.get(&asset.hash) {
        let rel_seen_path = seen_path.relative_to(&state.input_prefix)?;

        debug!("Duplicate asset found: {} -> {}", rel_path, rel_seen_path);

        state
            .sync_state
            .event_tx
            .send(Event::Duplicate {
                input_name: state.input_name.clone(),
                path: rel_path.clone(),
                original_path: rel_seen_path,
            })
            .await?;

        return Ok(());
    }

    state
        .seen_hashes
        .insert(asset.hash.clone(), entry.path().into());

    let lockfile_entry = state
        .sync_state
        .existing_lockfile
        .get(&state.input_name, &asset.hash);

    let needs_sync = lockfile_entry.is_none()
        || matches!(
            state.sync_state.args.target,
            SyncTarget::Debug | SyncTarget::Studio
        );

    if needs_sync {
        let font_db = state.sync_state.font_db.clone();
        asset.process(font_db, state.bleed).await?;

        if let Some(asset_ref) = state
            .sync_state
            .target_backend
            .sync(state.sync_state.clone(), state.input_name.clone(), &asset)
            .await?
        {
            state
                .sync_state
                .event_tx
                .send(Event::Insert {
                    new: matches!(state.sync_state.args.target, SyncTarget::Cloud)
                        && lockfile_entry.is_none(),
                    input_name: state.input_name.clone(),
                    path: asset.path.clone(),
                    hash: asset.hash.clone(),
                    asset_ref,
                })
                .await?
        }
    } else if let Some(entry) = lockfile_entry {
        state
            .sync_state
            .event_tx
            .send(Event::Insert {
                new: false,
                input_name: state.input_name.clone(),
                path: asset.path.clone(),
                hash: asset.hash.clone(),
                asset_ref: AssetRef::Cloud(entry.asset_id),
            })
            .await?
    }

    Ok(())
}

struct SyncResults {
    new_lockfile: Lockfile,
    input_sources: HashMap<String, NodeSource>,
    dupe_count: u32,
    new_count: u32,
}

async fn collect_events(
    mut rx: Receiver<Event>,
    config: Config,
    dry_run: bool,
) -> anyhow::Result<SyncResults> {
    let mut new_lockfile = Lockfile::default();

    let mut input_sources: HashMap<String, NodeSource> = HashMap::new();
    for (input_name, input) in &config.inputs {
        for (rel_path, web_asset) in &input.web {
            input_sources
                .entry(input_name.clone())
                .or_default()
                .insert(rel_path.clone(), web_asset.clone().into());
        }
    }

    let mut new_count = 0;
    let mut dupe_count = 0;

    while let Some(event) = rx.recv().await {
        match event {
            Event::Insert {
                new,
                input_name,
                path,
                hash,
                asset_ref,
            } => {
                input_sources
                    .entry(input_name.clone())
                    .or_default()
                    .insert(path, asset_ref.clone());

                if let AssetRef::Cloud(id) = asset_ref {
                    new_lockfile.insert(&input_name, &hash, LockfileEntry { asset_id: id });
                }

                if new {
                    new_count += 1;

                    if !dry_run {
                        new_lockfile.write(None).await?;
                    }
                }
            }
            Event::Duplicate {
                input_name,
                path,
                original_path,
            } => {
                dupe_count += 1;

                // If it's a duplicate, then it exists in the map.
                let source = input_sources.get_mut(&input_name).unwrap();
                let original = source
                    .get(&original_path)
                    .expect("We marked a duplicate, but there was no source");

                source.insert(path, original.clone());
            }
        }
    }

    Ok(SyncResults {
        new_lockfile,
        input_sources,
        dupe_count,
        new_count,
    })
}
