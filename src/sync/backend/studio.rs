use crate::{
    asset::{Asset, AssetKind, ModelKind},
    config::Input,
    sync::SyncState,
};

use super::{BackendSyncResult, SyncBackend};
use anyhow::Context;
use log::{info, warn};
use roblox_install::RobloxStudio;
use std::{env, path::PathBuf, sync::Arc};
use tokio::fs;

pub struct StudioBackend {
    identifier: String,
    sync_path: PathBuf,
}

impl SyncBackend for StudioBackend {
    async fn new() -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let studio = RobloxStudio::locate()?;

        let current_dir = env::current_dir()?;
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
            fs::remove_dir_all(&sync_path).await?;
        }

        Ok(Self {
            identifier,
            sync_path,
        })
    }

    async fn sync(
        &self,
        state: Arc<SyncState>,
        input: &Input,
        asset: &Asset,
    ) -> anyhow::Result<Option<BackendSyncResult>> {
        if let AssetKind::Model(ModelKind::Animation(_)) = asset.kind {
            let existing_id = state
                .existing_lockfile
                .get(input.name.clone(), &asset.path)
                .and_then(|entry| {
                    if entry.hash == asset.hash {
                        Some(entry.asset_id)
                    } else {
                        None
                    }
                });

            return match existing_id {
                Some(id) => Ok(Some(BackendSyncResult::Studio(format!(
                    "rbxassetid://{}",
                    id
                )))),
                None => {
                    warn!("Animations cannot be synced in this context");
                    Ok(None)
                }
            };
        }

        let rel_path = asset.rel_path(&input.path.get_prefix())?;
        let target_path = self.sync_path.join(&rel_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&target_path, &asset.data).await?;

        Ok(Some(BackendSyncResult::Studio(format!(
            "rbxasset://{}/{}",
            self.identifier,
            target_path.display()
        ))))
    }
}
