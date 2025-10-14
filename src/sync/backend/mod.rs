use std::sync::Arc;

use super::SyncState;
use crate::asset::Asset;

pub mod cloud;
pub mod debug;
pub mod studio;

pub enum BackendSyncResult {
    Cloud(u64),
    Studio(String),
}

pub trait SyncBackend {
    async fn new() -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn sync(
        &self,
        state: Arc<SyncState>,
        input_name: String,
        asset: &Asset,
    ) -> anyhow::Result<Option<BackendSyncResult>>;
}
