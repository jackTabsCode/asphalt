use std::sync::Arc;

use super::SyncState;
use crate::asset::Asset;

pub mod cloud;
pub mod debug;
pub mod studio;

pub trait SyncBackend {
    async fn new() -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn sync(
        &self,
        state: Arc<SyncState>,
        input_name: String,
        asset: &Asset,
    ) -> Result<Option<AssetRef>, SyncError>;
}

pub enum AssetRef {
    Cloud(u64),
    Studio(String),
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("Fatal error: {0}")]
    Fatal(anyhow::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
