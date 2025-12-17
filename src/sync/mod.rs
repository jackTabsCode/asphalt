use crate::{
    asset::AssetRef,
    cli::{SyncArgs, SyncTarget},
    config::Config,
    lockfile::{Lockfile, LockfileEntry, RawLockfile},
    sync::codegen::NodeSource,
    web_api::WebApiClient,
};
use anyhow::{Context, Result, bail};
use indicatif::MultiProgress;
use log::{info, warn};
use relative_path::RelativePathBuf;
use resvg::usvg::fontdb;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    fs,
    sync::mpsc::{self, Receiver, Sender},
};
use walk::{DuplicateFile, WalkedFile};

mod backend;
mod codegen;
mod perform;
mod process;
mod walk;

pub struct SyncState {
    args: SyncArgs,
    existing_lockfile: Lockfile,
    event_tx: Sender<SyncEvent>,
    multi_progress: MultiProgress,
    font_db: Arc<fontdb::Database>,
    client: WebApiClient,
}

#[derive(Debug)]
pub struct SyncEvent {
    write_lockfile: bool,
    input_name: String,
    path: RelativePathBuf,
    hash: String,
    asset_ref: AssetRef,
}

pub async fn sync(multi_progress: MultiProgress, args: SyncArgs) -> Result<()> {
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

    let (event_tx, event_rx) = mpsc::channel::<SyncEvent>(100);

    let collector_handle = tokio::spawn({
        let config = config.clone();
        async move { collect_events(event_rx, config).await }
    });

    let state = Arc::new(SyncState {
        args: args.clone(),
        existing_lockfile,
        event_tx,
        multi_progress,
        font_db,
        client: WebApiClient::new(args.api_key, config.creator, args.expected_price),
    });

    let mut duplicate_assets = HashMap::<String, Vec<DuplicateFile>>::new();

    for (input_name, input) in &config.inputs {
        let walk_results = walk::walk(state.clone(), input_name.clone(), input).await?;

        let mut new_assets = Vec::with_capacity(walk_results.len());
        let mut dupe_count = 0;

        for result in walk_results {
            match result {
                WalkedFile::New(asset) => {
                    new_assets.push(asset);
                }
                WalkedFile::Existing(existing) => {
                    if args.dry_run {
                        continue;
                    }

                    state
                        .event_tx
                        .send(SyncEvent {
                            write_lockfile: false,
                            input_name: input_name.clone(),
                            path: existing.path,
                            hash: existing.hash,
                            asset_ref: AssetRef::Cloud(existing.entry.asset_id),
                        })
                        .await?;
                }
                WalkedFile::Duplicate(dupe) => {
                    if input.warn_each_duplicate {
                        warn!(
                            "Duplicate file found: {} (original at {})",
                            dupe.path, dupe.original_path
                        );
                    }

                    if args.dry_run {
                        continue;
                    }

                    dupe_count += 1;

                    duplicate_assets
                        .entry(input_name.clone())
                        .or_default()
                        .push(DuplicateFile {
                            original_path: dupe.original_path,
                            path: dupe.path,
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

        perform::perform(&processed_assets, state.clone(), input_name.clone()).await?;
    }

    drop(state);

    let (new_lockfile, mut inputs_to_sources) = collector_handle.await??;

    if matches!(args.target, SyncTarget::Cloud) {
        new_lockfile.write(None).await?;
    }

    for (input_name, dupes) in duplicate_assets {
        let source = inputs_to_sources.get_mut(&input_name).unwrap();

        for dupe in dupes {
            let original = source
                .get(&dupe.original_path)
                .expect("We marked a duplicate, but there was no source");
            source.insert(dupe.path, original.clone());
        }
    }

    for (input_name, source) in inputs_to_sources {
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

async fn collect_events(
    mut rx: Receiver<SyncEvent>,
    config: Config,
) -> Result<(Lockfile, HashMap<String, NodeSource>)> {
    let mut lockfile = Lockfile::default();

    let mut inputs_to_sources: HashMap<String, NodeSource> = HashMap::new();
    for (input_name, input) in &config.inputs {
        for (rel_path, web_asset) in &input.web {
            inputs_to_sources
                .entry(input_name.clone())
                .or_default()
                .insert(rel_path.clone(), web_asset.clone().into());
        }
    }

    while let Some(event) = rx.recv().await {
        inputs_to_sources
            .entry(event.input_name.clone())
            .or_default()
            .insert(event.path, event.asset_ref.clone());

        if let AssetRef::Cloud(id) = event.asset_ref {
            lockfile.insert(
                &event.input_name,
                &event.hash,
                LockfileEntry { asset_id: id },
            );
        }

        if event.write_lockfile {
            lockfile.write(None).await?;
        }
    }

    Ok((lockfile, inputs_to_sources))
}
