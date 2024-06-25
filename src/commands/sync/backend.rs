use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::Context;
use log::{info, warn};

use roblox_install::RobloxStudio;
use tokio::fs::create_dir_all;

use crate::asset::{Asset, AssetKind, ModelKind};

use super::state::SyncState;

pub enum SyncResult {
    Cloud(u64),
    Studio(String),
    None,
}

pub trait SyncBackend {
    async fn sync(
        &self,
        state: &mut SyncState,
        path: &str,
        asset: Asset,
    ) -> anyhow::Result<SyncResult>;
}

fn normalize_path(state: &SyncState, path: &str) -> anyhow::Result<String> {
    path.strip_prefix(state.asset_dir.to_str().unwrap())
        .context("Failed to strip asset directory prefix")
        .map(|s| s.to_string())
}

async fn sync_to_path(write_path: &Path, asset_path: &str, asset: Asset) -> anyhow::Result<()> {
    let mut asset_path = write_path.join(asset_path);
    asset_path.set_extension(asset.extension());

    let parent_path = asset_path
        .parent()
        .context("Asset should have a parent path")?;

    create_dir_all(parent_path)
        .await
        .with_context(|| format!("Failed to create asset folder {}", parent_path.display()))?;

    asset
        .write(&asset_path)
        .await
        .with_context(|| format!("Failed to write asset to {}", asset_path.display()))
}

pub struct CloudBackend;

impl SyncBackend for CloudBackend {
    async fn sync(
        &self,
        state: &mut SyncState,
        path: &str,
        asset: Asset,
    ) -> anyhow::Result<SyncResult> {
        let result = asset
            .upload(
                state.creator.clone(),
                state.api_key.clone(),
                state.cookie.clone(),
                None,
            )
            .await?;
        state.update_csrf(result.csrf);

        info!("Uploaded {path}");
        Ok(SyncResult::Cloud(result.asset_id))
    }
}

pub struct StudioBackend {
    sync_path: PathBuf,
}

impl StudioBackend {
    pub fn new() -> anyhow::Result<Self> {
        let studio = RobloxStudio::locate().context("Failed to get Roblox Studio path")?;
        let sync_path = studio.content_path().join(".asphalt");
        info!("Assets will be synced to: {}", sync_path.display());
        Ok(Self { sync_path })
    }
}

impl SyncBackend for StudioBackend {
    async fn sync(
        &self,
        state: &mut SyncState,
        path: &str,
        asset: Asset,
    ) -> anyhow::Result<SyncResult> {
        if let AssetKind::Model(ModelKind::Animation) = asset.kind() {
            warn!("Animations cannot be synced to Roblox Studio, skipping {path}");
            return Ok(SyncResult::None);
        }

        let asset_path = normalize_path(state, path).context("Failed to normalize asset path")?;
        sync_to_path(&self.sync_path, &asset_path, asset)
            .await
            .context("Failed to sync asset to Roblox Studio")?;

        info!("Synced {path}");
        Ok(SyncResult::Studio(format!(
            "rbxasset://.asphalt/{}",
            asset_path
        )))
    }
}

pub struct DebugBackend;

impl SyncBackend for DebugBackend {
    async fn sync(
        &self,
        state: &mut SyncState,
        path: &str,
        asset: Asset,
    ) -> anyhow::Result<SyncResult> {
        let debug_path = env::current_dir()?.join(".asphalt-debug");
        let asset_path = normalize_path(state, path).context("Failed to normalize asset path")?;
        sync_to_path(&debug_path, &asset_path, asset)
            .await
            .context("Failed to sync asset")?;

        info!("Synced {path}");
        Ok(SyncResult::None)
    }
}
