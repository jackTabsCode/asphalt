use std::path::{Path, PathBuf};

use anyhow::Context;
use tokio::fs;

use crate::asset::Asset;

use super::state::SyncState;

pub mod cloud;
pub mod debug;
pub mod studio;

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
        asset: &Asset,
    ) -> anyhow::Result<SyncResult>;
}

fn asset_path(asset_dir: &str, path: &str, ext: &str) -> anyhow::Result<PathBuf> {
    let asset_dir = asset_dir.replace('\\', "/");
    let path = path.replace('\\', "/");

    let asset_dir = if asset_dir.ends_with('/') {
        asset_dir
    } else {
        format!("{}/", asset_dir)
    };

    let stripped_path = if path.starts_with(&asset_dir) {
        path.strip_prefix(&asset_dir).unwrap_or(&path)
    } else {
        let asset_dir_no_slash = asset_dir.trim_end_matches('/');
        if path.starts_with(asset_dir_no_slash) {
            path.strip_prefix(asset_dir_no_slash)
                .unwrap_or(&path)
                .trim_start_matches('/')
        } else {
            &path
        }
    };

    Ok(PathBuf::from(stripped_path).with_extension(ext))
}

async fn write_to_path(dest_path: &Path, asset_path: &Path, data: &[u8]) -> anyhow::Result<()> {
    let write_path = dest_path.join(asset_path);
    let parent_path = write_path
        .parent()
        .context("Asset should have a parent path")?;

    fs::create_dir_all(parent_path)
        .await
        .with_context(|| format!("Failed to create asset folder {}", parent_path.display()))?;

    fs::write(&write_path, data)
        .await
        .with_context(|| format!("Failed to write asset to {}", asset_path.display()))
}
