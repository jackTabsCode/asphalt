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
