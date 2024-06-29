use std::{env, path::PathBuf};

use anyhow::Context;
use log::{debug, info};
use tokio::fs::remove_dir_all;

use crate::{
    asset::Asset,
    commands::sync::{
        backend::{asset_path, sync_to_path},
        state::SyncState,
    },
};

use super::{SyncBackend, SyncResult};

pub struct DebugBackend {
    sync_path: PathBuf,
}

impl DebugBackend {
    pub async fn new() -> anyhow::Result<Self> {
        let debug_path = env::current_dir()?.join(".asphalt-debug");
        info!("Assets will be synced to: {}", debug_path.display());

        if debug_path.exists() {
            debug!("Removing existing folder...");
            remove_dir_all(&debug_path)
                .await
                .context("Failed to remove existing folder")?;
        }

        Ok(Self {
            sync_path: debug_path,
        })
    }
}

impl SyncBackend for DebugBackend {
    async fn sync(
        &self,
        state: &mut SyncState,
        path: &str,
        asset: Asset,
    ) -> anyhow::Result<SyncResult> {
        let asset_path = asset_path(state.asset_dir.to_str().unwrap(), path, asset.extension())
            .context("Failed to normalize asset path")?;
        sync_to_path(&self.sync_path, &asset_path, asset)
            .await
            .context("Failed to sync asset")?;

        info!("Synced {path}");
        Ok(SyncResult::None)
    }
}
