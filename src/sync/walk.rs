use crate::{
    asset::{self, Asset, AssetRef},
    cli::SyncTarget,
    config::Config,
};
use anyhow::Context;
use dashmap::DashMap;
use fs_err::tokio as fs;
use log::{debug, warn};
use relative_path::PathExt;
use std::{path::PathBuf, sync::Arc};
use tokio::task::JoinSet;
use walkdir::WalkDir;

struct InputState {
    sync_state: Arc<super::State>,
    input_name: String,
    input_prefix: PathBuf,
    seen_hashes: Arc<DashMap<String, PathBuf>>,
    bleed: bool,
}

pub async fn walk(state: super::State, config: &Config) {
    let state = Arc::new(state);

    for (input_name, input) in &config.inputs {
        let prefix = input.include.get_prefix();
        let entries = WalkDir::new(&prefix)
            .into_iter()
            .filter_entry(|entry| prefix == entry.path() || input.include.is_match(entry.path()))
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry
                        .path()
                        .extension()
                        .is_some_and(asset::is_supported_extension)
            })
            .collect::<Vec<_>>();

        let ctx = Arc::new(InputState {
            sync_state: state.clone(),
            input_name: input_name.clone(),
            input_prefix: prefix,
            seen_hashes: Arc::new(DashMap::with_capacity(entries.len())),
            bleed: input.bleed,
        });

        let mut join_set = JoinSet::new();

        for entry in entries {
            let ctx = ctx.clone();

            join_set.spawn(async move {
                if let Err(e) = process_entry(ctx, &entry).await {
                    warn!("Skipping file {}: {e:?}", entry.path().display());
                }
            });
        }

        while join_set.join_next().await.is_some() {}
    }
}

async fn process_entry(state: Arc<InputState>, entry: &walkdir::DirEntry) -> anyhow::Result<()> {
    debug!("Handling entry: {}", entry.path().display());

    let data = fs::read(entry.path()).await?;
    let rel_path = entry.path().relative_to(&state.input_prefix)?;

    let mut asset = Asset::new(rel_path.clone(), data)
        .await
        .context("Failed to create asset")?;

    if let Some(seen_path) = state.seen_hashes.get(&asset.hash) {
        let rel_seen_path = seen_path.relative_to(&state.input_prefix)?;

        debug!("Duplicate asset found: {} -> {}", rel_path, rel_seen_path);

        state
            .sync_state
            .event_tx
            .send(super::Event::Duplicate {
                input_name: state.input_name.clone(),
                path: rel_path.clone(),
                original_path: rel_seen_path,
            })
            .await?;

        return Ok(());
    }

    state
        .seen_hashes
        .insert(asset.hash.clone(), entry.path().into());

    let lockfile_entry = state
        .sync_state
        .existing_lockfile
        .get(&state.input_name, &asset.hash);

    let needs_sync = lockfile_entry.is_none()
        || matches!(
            state.sync_state.args.target,
            SyncTarget::Debug | SyncTarget::Studio
        );

    if needs_sync {
        let font_db = state.sync_state.font_db.clone();
        asset.process(font_db, state.bleed).await?;

        let asset_ref = state
            .sync_state
            .target_backend
            .sync(state.sync_state.clone(), state.input_name.clone(), &asset)
            .await?;

        state
            .sync_state
            .event_tx
            .send(super::Event::Process {
                new: matches!(state.sync_state.args.target, SyncTarget::Cloud)
                    && lockfile_entry.is_none(),
                input_name: state.input_name.clone(),
                path: asset.path.clone(),
                hash: asset.hash.clone(),
                asset_ref,
            })
            .await?
    } else if let Some(entry) = lockfile_entry {
        state
            .sync_state
            .event_tx
            .send(super::Event::Process {
                new: false,
                input_name: state.input_name.clone(),
                path: asset.path.clone(),
                hash: asset.hash.clone(),
                asset_ref: Some(AssetRef::Cloud(entry.asset_id)),
            })
            .await?
    }

    Ok(())
}
