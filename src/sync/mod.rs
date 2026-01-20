use crate::{
    asset::{Asset, AssetRef},
    cli::{SyncArgs, SyncTarget},
    config::Config,
    hash::Hash,
    lockfile::{LockfileEntry, RawLockfile},
    sync::{backend::Backend, collect::collect_events},
};
use anyhow::{Context, bail};
use fs_err::tokio as fs;
use indicatif::MultiProgress;
use log::info;
use relative_path::RelativePathBuf;
use resvg::usvg::fontdb;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::mpsc::{self};

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

    let existing_lockfile = RawLockfile::read_from(&config.project_dir)
        .await?
        .into_lockfile()?;

    let font_db = Arc::new({
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    });

    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();

    let collector_handle = tokio::spawn({
        let inputs = config.inputs.clone();
        let project_dir = config.project_dir.clone();
        async move { collect_events(event_rx, target, inputs, mp, &project_dir).await }
    });

    let params = walk::Params {
        target,
        existing_lockfile,
        font_db,
        backend: {
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
                SyncTarget::Studio => {
                    Some(TargetBackend::Studio(backend::Studio::new(params).await?))
                }
            }
        },
    };

    walk::walk(params, &config, &event_tx).await;
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

            let output_path = config.project_dir.join(&input.output_path);
            fs::create_dir_all(&output_path).await?;
            fs::write(output_path.join(format!("{input_name}.{ext}")), code).await?;
        }
    }

    if results.any_failed {
        bail!("Some assets failed to sync")
    }

    Ok(())
}
