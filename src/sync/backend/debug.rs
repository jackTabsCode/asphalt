use super::{AssetRef, Backend};
use crate::{asset::Asset, lockfile::LockfileEntry, sync::backend::Params};
use anyhow::Context;
use fs_err::tokio as fs;
use log::info;
use std::path::PathBuf;

pub struct Debug {
    sync_path: PathBuf,
}

impl Backend for Debug {
    async fn new(params: Params) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let debug_path = params.project_dir.join(".asphalt-debug");
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
        asset: &Asset,
        lockfile_entry: Option<&LockfileEntry>,
    ) -> anyhow::Result<Option<AssetRef>> {
        let target_path = asset.path.to_logical_path(&self.sync_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directories")?;
        }

        fs::write(&target_path, &asset.data)
            .await
            .with_context(|| format!("Failed to write asset to {}", target_path.display()))?;

        Ok(lockfile_entry.map(Into::into))
    }
}
