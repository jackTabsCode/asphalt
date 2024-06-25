use std::path::Path;

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

fn normalize_asset_path(state: &SyncState, path: &str) -> anyhow::Result<String> {
    path.strip_prefix(state.asset_dir.to_str().unwrap())
        .context("Failed to strip asset directory prefix")
        .map(|s| s.to_string())
}

async fn sync_to_path(write_path: &Path, asset_path: &str, asset: Asset) -> anyhow::Result<()> {
    let mut asset_path = write_path.join(asset_path);
    asset_path.set_extension(asset.extension());

    let parent_path = asset_path
        .parent()
        .context("Asset should have a parent path")?;

    create_dir_all(parent_path)
        .await
        .with_context(|| format!("Failed to create asset folder {}", parent_path.display()))?;

    write(&asset_path, asset.data())
        .await
        .with_context(|| format!("Failed to write asset to {}", asset_path.display()))
}
