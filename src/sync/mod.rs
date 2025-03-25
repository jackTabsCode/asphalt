use crate::{
    cli::SyncArgs,
    config::Config,
    lockfile::{Lockfile, LockfileEntry},
};
use anyhow::Context;
use env_logger::Logger;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::debug;
use resvg::usvg::fontdb::Database;
use std::{env, sync::Arc};
use tokio::sync::mpsc;

mod backend;
mod process;
mod walk;

pub struct SyncState {
    args: SyncArgs,
    config: Config,
    existing_lockfile: Lockfile,
    lockfile_tx: mpsc::Sender<(String, LockfileEntry)>,

    multi_progress: MultiProgress,
    font_db: Arc<Database>,
    api_key: String,
    cookie: Option<String>,
    csrf: Option<String>,
}

pub async fn sync(logger: Logger, args: SyncArgs) -> anyhow::Result<()> {
    let config = Config::read()?;
    let lockfile = Lockfile::read().await?;

    let api_key = env::var("ASPHALT_API_KEY").context("ASPHALT_API_KEY variable must be set to use Asphalt.\nAcquire one here: https://create.roblox.com/dashboard/credentials")?;

    let cookie = rbx_cookie::get();

    let multi_progress = MultiProgress::new();
    LogWrapper::new(multi_progress.clone(), logger).try_init()?;

    let font_db = Arc::new({
        let mut db = Database::new();
        db.load_system_fonts();
        db
    });

    let mut new_lockfile = lockfile.clone();
    let (tx, mut rx) = mpsc::channel(100);

    tokio::spawn(async move {
        while let Some((path, entry)) = rx.recv().await {
            new_lockfile.entries.insert(path, entry);
            new_lockfile.write(None).await.unwrap();
        }
    });

    let state = Arc::new(SyncState {
        args,
        multi_progress,
        config: config.clone(),
        existing_lockfile: lockfile.clone(),
        lockfile_tx: tx,
        font_db,
        api_key,
        cookie,
        csrf: None,
    });

    let mut handles = Vec::new();

    for input in config.inputs {
        let state = state.clone();

        let handle = tokio::spawn(async move {
            debug!("Walking input: {}", input.name);
            let paths = walk::walk(state.clone(), &input).await?;

            debug!("Discovered {} files, starting processing", paths.len());
            process::process_input(state, &input, paths).await
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    Ok(())
}
