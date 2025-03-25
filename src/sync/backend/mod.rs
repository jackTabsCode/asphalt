use super::SyncState;
use crate::asset::Asset;
use anyhow::Context;
use std::path::{Path, PathBuf};

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
