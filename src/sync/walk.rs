use super::SyncState;
use crate::{
    asset::Asset, cli::SyncTarget, config::Input, lockfile::LockfileEntry,
    progress_bar::ProgressBar,
};
use anyhow::Context;
use dashmap::DashMap;
use fs_err::tokio as fs;
use futures::stream::{self, StreamExt};
use log::debug;
use std::{path::PathBuf, sync::Arc};
use tokio::task::spawn_blocking;
use walkdir::WalkDir;

#[derive(Clone)]
struct WalkCtx {
    state: Arc<SyncState>,
    input_name: String,
    seen_hashes: Arc<DashMap<String, PathBuf>>,
    pb: ProgressBar,
}

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

    let seen_hashes = Arc::new(DashMap::<String, PathBuf>::with_capacity(total_files));

    let ctx = WalkCtx {
        state,
        input_name,
        seen_hashes,
        pb,
    };

    let results = stream::iter(entries)
        .map(|path| {
            let ctx = ctx.clone();

            async move {
                let result =
                    walk_file(ctx.state, ctx.input_name, path.clone(), ctx.seen_hashes).await;

                ctx.pb.inc(1);

                match result {
                    Ok(res) => Some(res),
                    Err(err) => {
                        debug!("Skipping file {}: {:?}", path.display(), err);
                        None
                    }
                }
            }
        })
        .buffer_unordered(100)
        .filter_map(|result| async move { result })
        .collect::<Vec<_>>()
        .await;

    ctx.pb.finish();

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
    let path_clone = path.clone();
    let asset = spawn_blocking(move || Asset::new(path_clone, data))
        .await
        .context("Failed to create asset")??;

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
