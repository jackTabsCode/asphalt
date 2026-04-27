use super::{AssetRef, Backend};
use crate::{
    asset::{Asset, AssetType},
    hash::Hash,
    lockfile::LockfileEntry,
    sync::backend::Params,
};
use anyhow::Context;
use fs_err::tokio as fs;
use log::{debug, info, warn};
use relative_path::RelativePathBuf;
use roblox_install::RobloxStudio;
use std::{collections::HashSet, env, path::PathBuf};

pub struct Studio {
    identifier: String,
    sync_path: PathBuf,
}

impl Backend for Studio {
    async fn new(params: Params) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let (identifier, sync_path) = if env::var("ASPHALT_TEST").is_ok() {
            let identifier = ".asphalt-test".to_string();
            let sync_path = params.project_dir.join(&identifier);
            (identifier, sync_path)
        } else {
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
            (identifier, sync_path)
        };

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
                Some(entry) => Ok(Some(AssetRef::Cloud(entry.asset_id))),
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
            "{}/{}",
            self.identifier, rel_target_path
        ))))
    }
}

impl Studio {
    /// Reconstruct an `AssetRef` for a previously-synced hash without re-syncing.
    pub fn ref_for_hash(&self, hash: &Hash, ext: &str) -> AssetRef {
        let rel = RelativePathBuf::from(&hash.to_string()).with_extension(ext);
        AssetRef::Studio(format!("{}/{}", self.identifier, rel))
    }

    /// Remove files from the sync folder whose hash is no longer referenced.
    pub async fn clean_orphans(&self, valid_hashes: &HashSet<Hash>) -> anyhow::Result<()> {
        let Ok(mut entries) = tokio::fs::read_dir(&self.sync_path).await else {
            return Ok(());
        };

        let valid_stems: HashSet<String> = valid_hashes.iter().map(|h| h.to_string()).collect();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if !valid_stems.contains(stem) {
                debug!("Removing orphaned file: {}", path.display());
                let _ = tokio::fs::remove_file(&path).await;
            }
        }

        Ok(())
    }
}
