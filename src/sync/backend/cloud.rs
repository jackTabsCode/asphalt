use super::SyncBackend;
use crate::{
    asset::{Asset, AssetRef},
    sync::{SyncState, backend::SyncError},
    web_api::UploadError,
};
use anyhow::anyhow;
use std::sync::Arc;

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
        match state.client.upload(asset).await {
            Ok(id) => Ok(Some(AssetRef::Cloud(id))),
            Err(UploadError::Fatal { message, .. }) => Err(SyncError::Fatal(anyhow!(message))),
            Err(UploadError::Other(e)) => Err(SyncError::Fatal(e)),
        }
    }
}
