use std::{env, path::PathBuf};

use anyhow::Context;
use log::{debug, info, warn};
use roblox_install::RobloxStudio;
use tokio::fs::remove_dir_all;

use crate::{
    asset::{Asset, AssetKind, ModelKind},
    commands::sync::{
        backend::{asset_path, sync_to_path},
        state::SyncState,
    },
};

use super::{SyncBackend, SyncResult};

pub struct StudioBackend {
    identifier: String,
    sync_path: PathBuf,
}

impl StudioBackend {
    pub async fn new() -> anyhow::Result<Self> {
        let studio = RobloxStudio::locate().context(
            "Failed to locate Roblox Studio, please set the ROBLOX_STUDIO_PATH \
            environment variable",
        )?;

        // Get current directory name and convert to kebab-case
        let current_dir = env::current_dir().context("Failed to get current directory")?;
        let name = current_dir
            .file_name()
            .and_then(|s| s.to_str())
            .context("Failed to get current directory name")?;

        let project_name = name
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-");

        let identifier = format!(".asphalt-{}", project_name);
        let sync_path = studio.content_path().join(&identifier);
        info!("Assets will be synced to: {}", sync_path.display());

        if sync_path.exists() {
            debug!("Removing existing folder...");
            remove_dir_all(&sync_path)
                .await
                .context("Failed to remove existing folder")?;
        }

        Ok(Self {
            identifier,
            sync_path,
        })
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
            let existing = state.existing_lockfile.entries.get(path).and_then(|entry| {
                if entry.hash == asset.hash() {
                    Some(entry)
                } else {
                    None
                }
            });

            if let Some(existing_value) = existing {
                return Ok(SyncResult::Studio(format!(
                    "rbxassetid://{}",
                    existing_value.asset_id
                )));
            }

            warn!("Animations cannot be synced as a file, please upload it first using the 'cloud' target");
            return Ok(SyncResult::None);
        }

        let asset_path = asset_path(state.asset_dir.to_str().unwrap(), path, asset.extension())
            .context("Failed to normalize asset path")?;
        sync_to_path(&self.sync_path, &asset_path, asset)
            .await
            .context("Failed to sync asset to Roblox Studio")?;

        info!("Synced {path}");
        Ok(SyncResult::Studio(format!(
            "rbxasset://{}/{}",
            self.identifier,
            asset_path.display()
        )))
    }
}
