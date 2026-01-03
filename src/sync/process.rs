use anyhow::Context;
use fs_err::tokio as fs;
use log::{debug, warn};
use relative_path::PathExt;
use resvg::usvg::fontdb;
use std::{
    collections::{HashMap, hash_map},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    sync::{Mutex, Semaphore, mpsc::UnboundedSender},
    task::JoinSet,
};

use crate::{
    asset::Asset, cli::SyncTarget, config::Input, lockfile::Lockfile, sync::TargetBackend,
};

pub struct WalkedInput {
    pub name: String,
    pub input: Input,
    pub paths: Vec<PathBuf>,
}

pub struct State {
    pub target: SyncTarget,
    pub existing_lockfile: Lockfile,
    pub font_db: Arc<fontdb::Database>,
    pub backend: Option<TargetBackend>,
}

pub async fn process_inputs(
    state: State,
    list: Vec<WalkedInput>,
    tx: &UnboundedSender<super::Event>,
) {
    let state = Arc::new(state);
    let seen_hashes = Arc::new(Mutex::new(HashMap::new()));

    let mut join_set = JoinSet::new();
    let semaphore = Arc::new(Semaphore::new(50));

    for walked_input in list {
        for path in walked_input.paths {
            let state = state.clone();
            let seen_hashes = seen_hashes.clone();
            let semaphore = semaphore.clone();
            let tx = tx.clone();
            let input_name = walked_input.name.clone();
            let input = walked_input.input.clone();

            join_set.spawn(async move {
                let _permit = semaphore.acquire_owned().await.unwrap();

                tx.send(super::Event::Processing(path.clone())).unwrap();

                if let Err(e) = process_path(
                    state.clone(),
                    input_name,
                    &input,
                    &path,
                    &tx,
                    seen_hashes.clone(),
                )
                .await
                {
                    warn!("Failed to process file {}: {e:?}", path.display());
                    tx.send(super::Event::Failed(path.clone())).unwrap();
                }
            });
        }
    }

    while join_set.join_next().await.is_some() {}
}

async fn process_path(
    state: Arc<State>,
    input_name: String,
    input: &Input,
    path: &Path,
    tx: &UnboundedSender<super::Event>,
    seen_hashes: Arc<Mutex<HashMap<String, PathBuf>>>,
) -> anyhow::Result<()> {
    debug!("Handling entry: {}", path.display());

    let prefix = input.include.get_prefix();
    let rel_path = path.relative_to(&prefix)?;

    let data = fs::read(path).await?;

    let asset = Asset::new(rel_path.clone(), data, state.font_db.clone(), input.bleed)
        .await
        .context("Failed to create asset")?;

    let lockfile_entry = state.existing_lockfile.get(&input_name, &asset.hash);

    {
        let mut seen_hashes = seen_hashes.lock().await;

        match seen_hashes.entry(asset.hash.clone()) {
            hash_map::Entry::Occupied(entry) => {
                let seen_path = entry.get();
                let rel_seen_path = seen_path.relative_to(prefix)?;

                debug!("Duplicate asset found: {} -> {}", rel_path, rel_seen_path);

                let event = super::Event::Finished {
                    state: super::EventState::Duplicate,
                    input_name: input_name.clone(),
                    path: path.into(),
                    rel_path: rel_path.clone(),
                    asset_ref: lockfile_entry.map(Into::into),
                    hash: asset.hash.clone(),
                };
                tx.send(event).unwrap();

                return Ok(());
            }
            hash_map::Entry::Vacant(_) => {
                seen_hashes.insert(asset.hash.clone(), path.into());
            }
        }
    }

    let always_target = matches!(state.target, SyncTarget::Studio | SyncTarget::Debug);
    let is_new = always_target || lockfile_entry.is_none();

    let asset_ref = match state.backend {
        Some(ref backend) => backend.sync(&asset, lockfile_entry).await?,
        None => lockfile_entry.map(Into::into),
    };

    let event = super::Event::Finished {
        state: super::EventState::Synced { new: is_new },
        input_name: input_name.clone(),
        path: path.into(),
        rel_path: asset.path.clone(),
        hash: asset.hash.clone(),
        asset_ref,
    };
    tx.send(event).unwrap();

    Ok(())
}
