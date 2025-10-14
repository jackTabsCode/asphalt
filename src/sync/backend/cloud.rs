use super::{BackendSyncResult, SyncBackend};
use crate::{asset::Asset, sync::SyncState};
use std::sync::Arc;
use tokio::time;

pub struct CloudBackend;

impl SyncBackend for CloudBackend {
    async fn new() -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self)
    }

    async fn sync(
        &self,
        state: Arc<SyncState>,
        _input_name: String,
        asset: &Asset,
    ) -> anyhow::Result<Option<BackendSyncResult>> {
        if cfg!(feature = "mock_cloud") {
            time::sleep(time::Duration::from_secs(1)).await;
            return Ok(Some(BackendSyncResult::Cloud(1337)));
        }

        let asset_id = state.client.upload(asset).await?;

        Ok(Some(BackendSyncResult::Cloud(asset_id)))
    }
}
