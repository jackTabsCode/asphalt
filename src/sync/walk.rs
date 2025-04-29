use super::SyncState;
use crate::{asset::Asset, cli::SyncTarget, config::Input, lockfile::LockfileEntry};
use fs_err::tokio as fs;
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use walkdir::WalkDir;

pub async fn walk(
    state: Arc<SyncState>,
    input_name: String,
    input: &Input,
) -> anyhow::Result<Vec<WalkResult>> {
    let mut seen_hashes = HashMap::<String, PathBuf>::new();

    let prefix = input.path.get_prefix();

    let prefix_display = prefix.to_string_lossy().to_string();
    let progress_bar = state
        .multi_progress
        .add(
            ProgressBar::new_spinner()
                .with_prefix(prefix_display)
                .with_style(
                    ProgressStyle::default_spinner()
                        .template("{prefix:.bold}: {spinner} {msg}")
                        .unwrap(),
                ),
        )
        .with_message("Collecting files");

    let entries = WalkDir::new(prefix)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| input.path.is_match(entry.path()) && entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();

    let mut res = Vec::with_capacity(entries.len());

    for path in entries {
        progress_bar.set_message(format!("Reading {}", path.display()));
        progress_bar.tick();

        match walk_file(
            state.clone(),
            input_name.clone(),
            path.clone(),
            &mut seen_hashes,
        )
        .await
        {
            Ok(result) => res.push(result),
            Err(err) => {
                warn!("Skipping file {}: {:?}", path.display(), err);
            }
        }
    }

    progress_bar.set_message("Done reading files");

    Ok(res)
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
    seen_hashes: &mut HashMap<String, PathBuf>,
) -> anyhow::Result<WalkResult> {
    let data = fs::read(&path).await?;
    let asset = Asset::new(path.clone(), data)?;

    let seen = seen_hashes.get(&asset.hash);
    if let Some(seen_path) = seen {
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
