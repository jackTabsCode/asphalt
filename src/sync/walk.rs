use super::SyncState;
use crate::{
    asset::Asset, cli::SyncTarget, config::Input, err::format_anyhow_chain, lockfile::LockfileEntry,
};
use fs_err::tokio as fs;
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use std::{path::PathBuf, sync::Arc};
use walkdir::WalkDir;

pub async fn walk(
    state: Arc<SyncState>,
    input_name: String,
    input: &Input,
) -> anyhow::Result<Vec<WalkFileResult>> {
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

    let mut res = Vec::new();

    for path in entries {
        progress_bar.set_message(format!("Reading {}", path.display()));
        progress_bar.tick();

        match walk_file(state.clone(), input_name.clone(), path.clone()).await {
            Ok(result) => res.push(result),
            Err(err) => {
                warn!(
                    "Skipping file {}: {}",
                    path.display(),
                    format_anyhow_chain(&err)
                );
            }
        }
    }

    progress_bar.set_message("Done reading files");

    Ok(res)
}

pub enum WalkFileResult {
    NewAsset(Asset),
    ExistingAsset((PathBuf, LockfileEntry)),
}

async fn walk_file(
    state: Arc<SyncState>,
    input_name: String,
    path: PathBuf,
) -> anyhow::Result<WalkFileResult> {
    let data = fs::read(&path).await?;
    let asset = Asset::new(path.clone(), data)?;

    let entry = state.existing_lockfile.get(&input_name, &path);

    match (entry, &state.args.target) {
        (Some(entry), SyncTarget::Cloud) => {
            if asset.hash == entry.hash {
                Ok(WalkFileResult::ExistingAsset((path, entry.clone())))
            } else {
                Ok(WalkFileResult::NewAsset(asset))
            }
        }
        (Some(_), SyncTarget::Studio | SyncTarget::Debug) => Ok(WalkFileResult::NewAsset(asset)),
        (None, _) => Ok(WalkFileResult::NewAsset(asset)),
    }
}
