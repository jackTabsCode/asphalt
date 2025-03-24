use crate::{asset::AssetKind, cli::SyncArgs, config::Config, lockfile::Lockfile};
use env_logger::Logger;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::debug;
use resvg::usvg::fontdb::Database;
use std::{path::PathBuf, sync::Arc};

mod process;
mod walk;

pub struct WalkedFile {
    path: PathBuf,
    data: Vec<u8>,
    kind: AssetKind,
}

pub struct ProcessedFile {
    file: WalkedFile,
    changed: bool,
}

pub struct SyncState {
    config: Config,
    lockfile: Lockfile,
    multi_progress: MultiProgress,
    font_db: Arc<Database>,
}

pub async fn sync(logger: Logger, args: SyncArgs) -> anyhow::Result<()> {
    let config = Config::read()?;
    let lockfile = Lockfile::read().await?;

    let multi_progress = MultiProgress::new();
    LogWrapper::new(multi_progress.clone(), logger).try_init()?;

    let font_db = Arc::new({
        let mut db = Database::new();
        db.load_system_fonts();
        db
    });

    let state = Arc::new(SyncState {
        multi_progress,
        config: config.clone(),
        lockfile: lockfile.clone(),
        font_db,
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
