use super::{BackendSyncResult, SyncBackend};
use crate::{
    asset::{Asset, AssetType},
    sync::SyncState,
};
use anyhow::{Context, bail};
use fs_err::tokio as fs;
use log::{debug, info, warn};
use relative_path::RelativePathBuf;
use roblox_install::RobloxStudio;
use std::{env, path::PathBuf, sync::Arc};

pub struct StudioBackend {
    identifier: String,
    sync_path: PathBuf,
}

impl SyncBackend for StudioBackend {
    async fn new() -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let content_path = get_content_path()?;

        let cwd = env::current_dir()?;
        let cwd_name = cwd
            .file_name()
            .and_then(|s| s.to_str())
            .context("Failed to get current directory name")?;

        let project_name = cwd_name
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-");

        let identifier = format!(".asphalt-{project_name}");
        let sync_path = content_path.join(&identifier);

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
        asset: &Asset,
    ) -> anyhow::Result<Option<BackendSyncResult>> {
        if matches!(asset.ty, AssetType::Model(_) | AssetType::Animation) {
            return match state.existing_lockfile.get(&input_name, &asset.hash) {
                Some(entry) => Ok(Some(BackendSyncResult::Studio(format!(
                    "rbxassetid://{}",
                    entry.asset_id
                )))),
                None => {
                    warn!(
                        "Models and Animations cannot be synced to Studio without having been uploaded first"
                    );
                    Ok(None)
                }
            };
        }

        let rel_target_path = RelativePathBuf::from(&asset.hash).with_extension(&asset.ext);
        let target_path = rel_target_path.to_logical_path(&self.sync_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&target_path, &asset.data).await?;

        Ok(Some(BackendSyncResult::Studio(format!(
            "rbxasset://{}/{}",
            self.identifier, rel_target_path
        ))))
    }
}

fn get_content_path() -> anyhow::Result<PathBuf> {
    if let Ok(var) = env::var("ROBLOX_CONTENT_PATH") {
        let path = PathBuf::from(var);

        if path.exists() {
            debug!("Using environment variable content path: {path:?}");
            return Ok(path);
        } else {
            bail!("Content path `{}` does not exist", path.display());
        }
    }

    let studio = RobloxStudio::locate()?;
    let path = studio.content_path();

    debug!("Using auto-detected content path: {path:?}");

    Ok(path.to_owned())
}
