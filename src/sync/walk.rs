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
use relative_path::{PathExt, RelativePathBuf};
use std::{path::PathBuf, sync::Arc};
use tokio::task::spawn_blocking;
use walkdir::WalkDir;

#[derive(Clone)]
struct WalkCtx {
    state: Arc<SyncState>,
    input_name: String,
    input_prefix: PathBuf,
    seen_hashes: Arc<DashMap<String, PathBuf>>,
    pb: ProgressBar,
}

pub async fn walk(
    state: Arc<SyncState>,
    input_name: String,
    input: &Input,
) -> anyhow::Result<Vec<WalkedFile>> {
    let input_prefix = input.path.get_prefix();

    let entries = WalkDir::new(&input_prefix)
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
        input_prefix,
    };

    let results = stream::iter(entries)
        .map(|path| {
            let ctx = ctx.clone();

            async move {
                let result = walk_file(&ctx, path.clone()).await;

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

pub struct ExistingFile {
    pub path: RelativePathBuf,
    pub hash: String,
    pub entry: LockfileEntry,
}

pub struct DuplicateFile {
    pub path: RelativePathBuf,
    pub original_path: RelativePathBuf,
}

pub enum WalkedFile {
    New(Asset),
    Existing(ExistingFile),
    Duplicate(DuplicateFile),
}

async fn walk_file(ctx: &WalkCtx, path: PathBuf) -> anyhow::Result<WalkedFile> {
    let data = fs::read(&path).await?;
    let rel_path = path.relative_to(&ctx.input_prefix)?;

    let rel_path_clone = rel_path.clone();
    let asset = spawn_blocking(move || Asset::new(rel_path_clone, data))
        .await
        .context("Failed to create asset")??;

    if let Some(seen_path) = ctx.seen_hashes.get(&asset.hash) {
        let rel_seen_path = seen_path.relative_to(&ctx.input_prefix)?;

        return Ok(WalkedFile::Duplicate(DuplicateFile {
            path: rel_path.clone(),
            original_path: rel_seen_path,
        }));
    }

    ctx.seen_hashes.insert(asset.hash.clone(), path.clone());

    let entry = ctx
        .state
        .existing_lockfile
        .get(&ctx.input_name, &asset.hash);

    match (entry, &ctx.state.args.target) {
        (Some(entry), SyncTarget::Cloud) => Ok(WalkedFile::Existing(ExistingFile {
            path: rel_path,
            hash: asset.hash.clone(),
            entry: entry.clone(),
        })),
        (Some(_), SyncTarget::Studio | SyncTarget::Debug) => Ok(WalkedFile::New(asset)),
        (None, _) => Ok(WalkedFile::New(asset)),
    }
}
