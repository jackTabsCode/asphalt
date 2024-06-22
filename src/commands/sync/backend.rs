use anyhow::Context;
use log::{info, warn};
use tokio::fs::{create_dir_all, write};

use roblox_install::RobloxStudio;

use crate::asset::{Asset, AssetKind, ModelKind};

use super::state::SyncState;

pub enum SyncResult {
    Upload(u64),
    Local(String),
    None,
}

pub trait SyncBackend {
    async fn sync(
        self,
        state: &mut SyncState,
        path: &str,
        asset: Asset,
    ) -> anyhow::Result<SyncResult>;
}

pub struct RobloxBackend;

impl SyncBackend for RobloxBackend {
    async fn sync(
        self,
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
        Ok(SyncResult::Upload(result.asset_id))
    }
}

pub struct LocalBackend;

impl SyncBackend for LocalBackend {
    async fn sync(
        self,
        state: &mut SyncState,
        path: &str,
        asset: Asset,
    ) -> anyhow::Result<SyncResult> {
        let studio = RobloxStudio::locate().context("Failed to get Roblox Studio path")?;

        if let AssetKind::Model(kind) = asset.kind {
            if let ModelKind::Animation = kind {
                warn!("Animations cannot be synced locally, skipping {path}");
                return Ok(SyncResult::None);
            }
        }

        let relative_path = path
            .strip_prefix(state.asset_dir.to_str().unwrap())
            .context("Failed to strip asset directory prefix")?;
        let asset_path = studio.content_path().join(".asphalt").join(relative_path);

        let parent_path = asset_path
            .parent()
            .context("Asset should have a parent path")?;

        create_dir_all(parent_path)
            .await
            .with_context(|| format!("Failed to create asset folder {}", parent_path.display()))?;

        write(&asset_path, asset.data)
            .await
            .with_context(|| format!("Failed to write asset to {}", asset_path.display()))?;

        info!("Synced {path}");
        Ok(SyncResult::Local(format!(
            "rbxasset://.asphalt/{}",
            relative_path
        )))
    }
}

pub struct NoneBackend;

impl SyncBackend for NoneBackend {
    async fn sync(self, _: &mut SyncState, path: &str, _: Asset) -> anyhow::Result<SyncResult> {
        info!("Synced {path}");
        Ok(SyncResult::None)
    }
}
