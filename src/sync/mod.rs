use crate::{
    asset::{Asset, AssetRef},
    cli::{SyncArgs, SyncTarget},
    config::Config,
    lockfile::{Lockfile, RawLockfile},
    sync::{backend::Backend, collect::collect_events},
};
use anyhow::{Context, bail};
use fs_err::tokio as fs;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{info, warn};
use relative_path::RelativePathBuf;
use resvg::usvg::fontdb;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender};

mod backend;
mod codegen;
mod collect;
mod walk;

pub struct State {
    args: SyncArgs,
    existing_lockfile: Lockfile,
    event_tx: Sender<Event>,
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
    Process {
        new: bool,
        input_name: String,
        path: RelativePathBuf,
        hash: String,
        asset_ref: Option<AssetRef>,
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

    let spinner = multi_progress.add(ProgressBar::new_spinner());
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner.set_message("Starting sync...");

    let collector_handle = tokio::spawn({
        let config = config.clone();
        let spinner = spinner.clone();
        async move { collect_events(event_rx, config, args.dry_run, spinner).await }
    });

    walk::walk(
        State {
            args: args.clone(),
            existing_lockfile,
            event_tx,
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

    spinner.tick();

    Ok(())
}
