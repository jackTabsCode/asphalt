use crate::{
    auth::Auth,
    cli::{SyncArgs, SyncTarget},
    config::Config,
    lockfile::{Lockfile, LockfileEntry},
};
use anyhow::{bail, Context};
use backend::BackendSyncResult;
use env_logger::Logger;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::debug;
use resvg::usvg::fontdb::Database;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{mpsc, RwLock};

mod backend;
mod perform;
mod process;
mod walk;

pub struct SyncResult {
    hash: String,
    path: PathBuf,
    input_name: String,
    backend: BackendSyncResult,
}

pub struct SyncState {
    client: reqwest::Client,
    args: SyncArgs,
    config: Config,
    existing_lockfile: Lockfile,
    result_tx: mpsc::Sender<SyncResult>,

    multi_progress: MultiProgress,
    font_db: Arc<Database>,
    auth: Auth,

    csrf: Arc<RwLock<Option<String>>>,
}

pub async fn sync(logger: Logger, args: SyncArgs) -> anyhow::Result<()> {
    if args.dry_run && !matches!(&args.target, Some(SyncTarget::Cloud)) {
        bail!("A dry run doesn't make sense in this context");
    }

    let config = Config::read()?;
    let lockfile = Lockfile::read().await?;
    let auth = Auth::new(args.api_key.clone())?;

    let multi_progress = MultiProgress::new();
    LogWrapper::new(multi_progress.clone(), logger).try_init()?;

    let font_db = Arc::new({
        let mut db = Database::new();
        db.load_system_fonts();
        db
    });

    let mut new_lockfile = lockfile.clone();
    let (result_tx, mut result_rx) = mpsc::channel::<SyncResult>(100);

    tokio::spawn(async move {
        while let Some(tx) = result_rx.recv().await {
            if let BackendSyncResult::Cloud(asset_id) = tx.backend {
                new_lockfile.insert(
                    tx.input_name,
                    &tx.path,
                    LockfileEntry {
                        asset_id,
                        hash: tx.hash,
                    },
                );

                let _ = new_lockfile
                    .write(None)
                    .await
                    .context("Failed to write lockfile");
            };

            // add to codegen
        }
    });

    let csrf = Arc::new(RwLock::new(None));

    let state = Arc::new(SyncState {
        client: reqwest::Client::new(),
        args,
        multi_progress,
        config: config.clone(),
        existing_lockfile: lockfile.clone(),
        result_tx,
        font_db,
        auth,
        csrf: csrf.clone(),
    });

    let mut handles: Vec<tokio::task::JoinHandle<anyhow::Result<()>>> = Vec::new();

    for input in config.inputs {
        let state = state.clone();

        handles.push(tokio::spawn(async move {
            debug!("Walking input {}", input.name);
            let mut assets = walk::walk(state.clone(), &input).await?;

            debug!(
                "Discovered {} files for input {}, starting processing",
                assets.len(),
                input.name
            );
            process::process(state.clone(), &input, &mut assets).await?;

            debug!("Starting perform for input {}", input.name);
            perform::perform(state, &input, &assets).await?;

            Ok(())
        }));
    }

    for handle in handles {
        handle.await??;
    }

    Ok(())
}
