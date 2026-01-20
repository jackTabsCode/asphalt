use super::{AssetRef, Backend};
use crate::{
    asset::{Asset, AssetType},
    lockfile::LockfileEntry,
    sync::backend::Params,
};
use anyhow::Context;
use fs_err::tokio as fs;
use log::{info, warn};
use relative_path::RelativePathBuf;
use roblox_install::RobloxStudio;
use std::{env, path::PathBuf};

pub struct Studio {
    identifier: String,
    sync_path: PathBuf,
}

impl Backend for Studio {
    async fn new(_: Params) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let studio = RobloxStudio::locate()?;
        let content_path = studio.content_path();

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
        asset: &Asset,
        lockfile_entry: Option<&LockfileEntry>,
    ) -> anyhow::Result<Option<AssetRef>> {
        if matches!(asset.ty, AssetType::Model(_) | AssetType::Animation) {
            return match lockfile_entry {
                Some(entry) => Ok(Some(AssetRef::Studio(format!(
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

        let rel_target_path =
            RelativePathBuf::from(&asset.hash.to_string()).with_extension(&asset.ext);
        let target_path = rel_target_path.to_logical_path(&self.sync_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&target_path, &asset.data).await?;

        Ok(Some(AssetRef::Studio(format!(
            "rbxasset://{}/{}",
            self.identifier, rel_target_path
        ))))
    }
}
