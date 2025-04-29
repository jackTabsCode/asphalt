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
    let mut num_dupes = 0;

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
            Err(WalkError::DuplicateAsset(original_path)) => {
                num_dupes += 1;

                if !state.args.suppress_duplicate_warnings {
                    warn!(
                        "Skipping duplicate file {} (original at {})",
                        path.display(),
                        original_path.display()
                    );
                }
            }
            Err(WalkError::Other(err)) => {
                warn!("Skipping file {}: {:?}", path.display(), err);
            }
        }
    }

    if num_dupes > 0 {
        warn!("{} duplicate assets found", num_dupes);
    }

    progress_bar.set_message("Done reading files");

    Ok(res)
}

pub enum WalkResult {
    New(Asset),
    Existing((PathBuf, String, LockfileEntry)),
}

#[derive(Debug)]
pub enum WalkError {
    DuplicateAsset(PathBuf),
    Other(anyhow::Error),
}

async fn walk_file(
    state: Arc<SyncState>,
    input_name: String,
    path: PathBuf,
    seen_hashes: &mut HashMap<String, PathBuf>,
) -> anyhow::Result<WalkResult, WalkError> {
    let data = match fs::read(&path).await {
        Ok(it) => it,
        Err(err) => return Err(WalkError::Other(err.into())),
    };
    let asset = match Asset::new(path.clone(), data) {
        Ok(it) => it,
        Err(err) => return Err(WalkError::Other(err)),
    };

    let seen = seen_hashes.get(&asset.hash);
    if let Some(seen_path) = seen {
        return Err(WalkError::DuplicateAsset(seen_path.clone()));
    }

    seen_hashes.insert(asset.hash.clone(), path.clone());

    let entry = state.existing_lockfile.get(&input_name, &asset.hash);

    match (entry, &state.args.target) {
        (Some(entry), SyncTarget::Cloud) => {
            Ok(WalkResult::Existing((path, asset.hash, entry.clone())))
        }
        (Some(_), SyncTarget::Studio | SyncTarget::Debug) => Ok(WalkResult::New(asset)),
        (None, _) => Ok(WalkResult::New(asset)),
    }
}
