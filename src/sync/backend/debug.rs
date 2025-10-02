use super::{BackendSyncResult, SyncBackend};
use crate::{asset::Asset, config::Input, sync::SyncState};
use anyhow::Context;
use fs_err::tokio as fs;
use log::info;
use std::{env, path::PathBuf, sync::Arc};

pub struct DebugBackend {
    sync_path: PathBuf,
}

impl SyncBackend for DebugBackend {
    async fn new() -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let debug_path = env::current_dir()
            .context("Failed to get current working directory")?
            .join(".asphalt-debug");
        info!("Assets will be synced to: {}", debug_path.display());

        if debug_path.exists() {
            fs::remove_dir_all(&debug_path)
                .await
                .context("Failed to remove existing folder")?;
        }

        fs::create_dir_all(&debug_path)
            .await
            .context("Failed to create debug directory")?;

        Ok(Self {
            sync_path: debug_path,
        })
    }

    async fn sync(
        &self,
        _state: Arc<SyncState>,
        _input_name: String,
        input: &Input,
        asset: &Asset,
    ) -> anyhow::Result<Option<BackendSyncResult>> {
        let rel_path = asset.rel_path(&input.path.get_prefix())?;
        let target_path = self.sync_path.join(&rel_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directories")?;
        }

        fs::write(&target_path, &asset.data)
            .await
            .with_context(|| format!("Failed to write asset to {}", target_path.display()))?;

        Ok(None)
    }
}
