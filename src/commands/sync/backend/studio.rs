use std::path::PathBuf;

use anyhow::Context;
use log::{info, warn};
use roblox_install::RobloxStudio;

use crate::{
    asset::{Asset, AssetKind, ModelKind},
    commands::sync::{
        backend::{normalize_asset_path, sync_to_path},
        state::SyncState,
    },
};

use super::{SyncBackend, SyncResult};

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

        let asset_path =
            normalize_asset_path(state, path).context("Failed to normalize asset path")?;
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
