use std::env;

use anyhow::Context;
use log::info;

use crate::{
    asset::Asset,
    commands::sync::{
        backend::{normalize_asset_path, sync_to_path},
        state::SyncState,
    },
};

use super::{SyncBackend, SyncResult};

pub struct DebugBackend;

impl SyncBackend for DebugBackend {
    async fn sync(
        &self,
        state: &mut SyncState,
        path: &str,
        asset: Asset,
    ) -> anyhow::Result<SyncResult> {
        let debug_path = env::current_dir()?.join(".asphalt-debug");
        let asset_path =
            normalize_asset_path(state, path).context("Failed to normalize asset path")?;
        sync_to_path(&debug_path, &asset_path, asset)
            .await
            .context("Failed to sync asset")?;

        info!("Synced {path}");
        Ok(SyncResult::None)
    }
}
