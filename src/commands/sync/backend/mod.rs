use std::path::{Path, PathBuf};

use anyhow::Context;
use tokio::fs::{create_dir_all, write};

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
        asset: Asset,
    ) -> anyhow::Result<SyncResult>;
}

fn asset_path(asset_dir: &str, path: &str, ext: &str) -> anyhow::Result<PathBuf> {
    let stripped_path_str = path
        .strip_prefix(asset_dir)
        .context("Failed to strip asset directory prefix")?;
    Ok(PathBuf::from(stripped_path_str).with_extension(ext))
}

async fn write_to_path(dest_path: &Path, asset_path: &Path, data: &[u8]) -> anyhow::Result<()> {
    let write_path = dest_path.join(asset_path);
    let parent_path = write_path
        .parent()
        .context("Asset should have a parent path")?;

    create_dir_all(parent_path)
        .await
        .with_context(|| format!("Failed to create asset folder {}", parent_path.display()))?;

    write(&write_path, data)
        .await
        .with_context(|| format!("Failed to write asset to {}", asset_path.display()))
}
