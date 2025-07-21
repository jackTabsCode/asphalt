use super::SyncState;
use crate::{
    asset::Asset, cli::SyncTarget, config::Input, lockfile::LockfileEntry,
    progress_bar::ProgressBar,
};
use dashmap::DashMap;
use fs_err::tokio as fs;
use futures::stream::{self, StreamExt};
use log::warn;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;
use walkdir::WalkDir;

pub async fn walk(
    state: Arc<SyncState>,
    input_name: String,
    input: &Input,
) -> anyhow::Result<Vec<WalkResult>> {
    let prefix = input.path.get_prefix();

    let entries = WalkDir::new(prefix)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| input.path.is_match(entry.path()) && entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();

    let total_files = entries.len();
    let pb = ProgressBar::new(
        state.multi_progress.clone(),
        &format!("Reading input \"{input_name}\""),
        total_files,
    );

    let semaphore = Arc::new(Semaphore::new(50));
    let seen_hashes = Arc::new(DashMap::<String, PathBuf>::with_capacity(total_files));

    let results = stream::iter(entries)
        .map(|path| {
            let state = state.clone();
            let input_name = input_name.clone();
            let seen_hashes = seen_hashes.clone();
            let semaphore = semaphore.clone();
            let pb = pb.clone();

            async move {
                let _permit = semaphore.acquire().await.unwrap();

                let result = walk_file(state, input_name, path.clone(), seen_hashes).await;

                pb.inc(1);

                match result {
                    Ok(res) => Some(res),
                    Err(err) => {
                        warn!("Skipping file {}: {:?}", path.display(), err);
                        None
                    }
                }
            }
        })
        .buffer_unordered(100)
        .filter_map(|result| async move { result })
        .collect::<Vec<_>>()
        .await;

    pb.finish();

    Ok(results)
}

pub struct ExistingResult {
    pub path: PathBuf,
    pub hash: String,
    pub entry: LockfileEntry,
}

pub struct DuplicateResult {
    pub path: PathBuf,
    pub original_path: PathBuf,
}

pub enum WalkResult {
    New(Asset),
    Existing(ExistingResult),
    Duplicate(DuplicateResult),
}

async fn walk_file(
    state: Arc<SyncState>,
    input_name: String,
    path: PathBuf,
    seen_hashes: Arc<DashMap<String, PathBuf>>,
) -> anyhow::Result<WalkResult> {
    let data = fs::read(&path).await?;
    let asset = Asset::new(path.clone(), data)?;

    if let Some(seen_path) = seen_hashes.get(&asset.hash) {
        return Ok(WalkResult::Duplicate(DuplicateResult {
            path: path.clone(),
            original_path: seen_path.clone(),
        }));
    }

    seen_hashes.insert(asset.hash.clone(), path.clone());

    let entry = state.existing_lockfile.get(&input_name, &asset.hash);

    match (entry, &state.args.target) {
        (Some(entry), SyncTarget::Cloud) => Ok(WalkResult::Existing(ExistingResult {
            path: path.clone(),
            hash: asset.hash.clone(),
            entry: entry.clone(),
        })),
        (Some(_), SyncTarget::Studio | SyncTarget::Debug) => Ok(WalkResult::New(asset)),
        (None, _) => Ok(WalkResult::New(asset)),
    }
}
