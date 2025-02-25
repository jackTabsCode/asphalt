use super::{SyncBackend, SyncResult};
use crate::{
    asset::Asset,
    commands::sync::{
        backend::{asset_path, write_to_path},
        state::SyncState,
    },
};
use anyhow::Context;
use log::{debug, info};
use std::{env, path::PathBuf};
use tokio::fs::remove_dir_all;

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
        asset: &Asset,
    ) -> anyhow::Result<SyncResult> {
        let path_buf = if path.starts_with("_spritesheets/") {
            PathBuf::from(path)
        } else {
            match asset_path(state.asset_dir.to_str().unwrap(), path, asset.extension()) {
                Ok(normalized) => normalized,
                Err(e) => {
                    debug!("Failed to normalize path {}: {}", path, e);
                    PathBuf::from(path)
                }
            }
        };

        write_to_path(&self.sync_path, &path_buf, asset.data())
            .await
            .context("Failed to sync asset")?;

        info!("Synced {path}");
        Ok(SyncResult::None)
    }
}
