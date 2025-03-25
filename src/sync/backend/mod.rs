use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::asset::Asset;

use super::SyncState;

mod cloud;
mod debug;
mod studio;

pub enum SyncResult {
    Cloud(u64),
    Studio(String),
}

pub trait SyncBackend {
    async fn sync(
        &self,
        state: &mut SyncState,
        asset: &Asset,
    ) -> anyhow::Result<Option<SyncResult>>;
}

fn relative_asset_path(input_path: &Path, asset_path: &Path, ext: &str) -> anyhow::Result<PathBuf> {
    let stripped_path_str = asset_path.strip_prefix(input_path)?;

    Ok(PathBuf::from(stripped_path_str).with_extension(ext))
}
