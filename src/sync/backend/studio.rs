use crate::{
    asset::{Asset, AssetType, ModelType},
    config::Input,
    sync::SyncState,
};

use super::{BackendSyncResult, SyncBackend};
use anyhow::Context;
use fs_err::tokio as fs;
use log::{info, warn};
use roblox_install::RobloxStudio;
use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

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

        let identifier = format!(".asphalt-{project_name}");
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
        input_name: String,
        input: &Input,
        asset: &Asset,
    ) -> anyhow::Result<Option<BackendSyncResult>> {
        if let AssetType::Model(ModelType::Animation(_)) = asset.ty {
            return match state.existing_lockfile.get(&input_name, &asset.hash) {
                Some(entry) => Ok(Some(BackendSyncResult::Studio(format!(
                    "rbxassetid://{}",
                    entry.asset_id
                )))),
                None => {
                    warn!("Animations cannot be synced in this context");
                    Ok(None)
                }
            };
        }

        let rel_path = asset.rel_path(&input.path.get_prefix())?;

        let parent_dir = rel_path.parent().unwrap_or_else(|| Path::new(""));
        let extension = rel_path.extension().unwrap().to_str().unwrap();
        let hash_filename = format!("{}.{}", asset.hash, extension);

        let hash_rel_path = parent_dir.join(hash_filename);
        let target_path = self.sync_path.join(&hash_rel_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&target_path, &asset.data).await?;

        let url_path = hash_rel_path.to_string_lossy().replace('\\', "/");

        Ok(Some(BackendSyncResult::Studio(format!(
            "rbxasset://{}/{}",
            self.identifier, url_path
        ))))
    }
}
