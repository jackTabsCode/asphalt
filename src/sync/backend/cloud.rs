use super::{AssetRef, SyncBackend};
use crate::{
    asset::Asset,
    sync::{SyncState, backend::SyncError},
    web_api::UploadError,
};
use anyhow::anyhow;
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
    ) -> Result<Option<AssetRef>, SyncError> {
        if cfg!(feature = "mock_cloud") {
            time::sleep(time::Duration::from_secs(1)).await;
            return Ok(Some(AssetRef::Cloud(1337)));
        }

        match state.client.upload(asset).await {
            Ok(id) => Ok(Some(AssetRef::Cloud(id))),
            Err(UploadError::Fatal { message, .. }) => Err(SyncError::Fatal(anyhow!(message))),
            Err(UploadError::Other(e)) => Err(SyncError::Fatal(e)),
        }
    }
}
