use crate::{
    asset::{self, Asset},
    cli::SyncTarget,
    config::Config,
    lockfile::Lockfile,
    sync::TargetBackend,
};
use anyhow::Context;
use fs_err::tokio as fs;
use log::{debug, warn};
use relative_path::PathExt;
use resvg::usvg::fontdb;
use std::{
    collections::{
        HashMap,
        hash_map::{self},
    },
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    sync::{Mutex, Semaphore, mpsc::Sender},
    task::JoinSet,
};
use walkdir::WalkDir;

pub struct Params {
    pub target: SyncTarget,
    pub existing_lockfile: Lockfile,
    pub font_db: Arc<fontdb::Database>,
    pub backend: Option<TargetBackend>,
}

struct InputState {
    params: Arc<Params>,
    input_name: String,
    input_prefix: PathBuf,
    seen_hashes: Arc<Mutex<HashMap<String, PathBuf>>>,
    bleed: bool,
}

pub async fn walk(params: Params, config: &Config, event_tx: &Sender<super::Event>) {
    let params = Arc::new(params);

    for (input_name, input) in &config.inputs {
        let state = Arc::new(InputState {
            params: params.clone(),
            input_name: input_name.clone(),
            input_prefix: input.include.get_prefix(),
            seen_hashes: Arc::new(Mutex::new(HashMap::new())),
            bleed: input.bleed,
        });

        let mut join_set = JoinSet::new();
        let semaphore = Arc::new(Semaphore::new(50));

        for entry in WalkDir::new(input.include.get_prefix())
            .into_iter()
            .filter_entry(|entry| {
                let path = entry.path();
                path == input.include.get_prefix() || input.include.is_match(path)
            })
        {
            let Ok(entry) = entry else { continue };

            let path = entry.into_path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension() else {
                continue;
            };
            if !asset::is_supported_extension(ext) {
                continue;
            }

            let ctx = state.clone();
            let semaphore = semaphore.clone();
            let event_tx = event_tx.clone();

            join_set.spawn(async move {
                let _permit = semaphore.acquire_owned().await.unwrap();
                if let Err(e) = process_entry(ctx.clone(), &path, &event_tx).await {
                    warn!("Skipping file {}: {e:?}", path.display());
                }
            });
        }

        while join_set.join_next().await.is_some() {}
    }
}

async fn process_entry(
    state: Arc<InputState>,
    path: &Path,
    tx: &Sender<super::Event>,
) -> anyhow::Result<()> {
    debug!("Handling entry: {}", path.display());

    let data = fs::read(path).await?;
    let rel_path = path.relative_to(&state.input_prefix)?;

    let asset = Asset::new(
        rel_path.clone(),
        data,
        state.params.font_db.clone(),
        state.bleed,
    )
    .await
    .context("Failed to create asset")?;

    let lockfile_entry = state
        .params
        .existing_lockfile
        .get(&state.input_name, &asset.hash);

    {
        let mut seen_hashes = state.seen_hashes.lock().await;

        match seen_hashes.entry(asset.hash.clone()) {
            hash_map::Entry::Occupied(entry) => {
                let seen_path = entry.get();
                let rel_seen_path = seen_path.relative_to(&state.input_prefix)?;

                debug!("Duplicate asset found: {} -> {}", rel_path, rel_seen_path);

                let event = super::Event {
                    ty: super::EventType::Duplicate,
                    input_name: state.input_name.clone(),
                    path: rel_path.clone(),
                    asset_ref: lockfile_entry.map(Into::into),
                    hash: asset.hash.clone(),
                };
                tx.send(event).await.unwrap();

                return Ok(());
            }
            hash_map::Entry::Vacant(_) => {
                seen_hashes.insert(asset.hash.clone(), path.into());
            }
        }
    }

    let always_target = matches!(state.params.target, SyncTarget::Studio | SyncTarget::Debug);
    let is_new = always_target || lockfile_entry.is_none();

    let asset_ref = match state.params.backend {
        Some(ref backend) => backend.sync(&asset, lockfile_entry).await?,
        None => lockfile_entry.map(Into::into),
    };

    let event = super::Event {
        ty: super::EventType::Synced { new: is_new },
        input_name: state.input_name.clone(),
        path: asset.path.clone(),
        hash: asset.hash.clone(),
        asset_ref,
    };
    tx.send(event).await.unwrap();

    Ok(())
}
