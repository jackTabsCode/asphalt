use crate::{
    asset::{Asset, AssetRef},
    config,
    lockfile::LockfileEntry,
};
use std::path::PathBuf;

mod cloud;
pub use cloud::Cloud;

mod debug;
pub use debug::Debug;

mod studio;
pub use studio::Studio;

pub trait Backend {
    async fn new(params: Params) -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn sync(
        &self,
        asset: &Asset,
        lockfile_entry: Option<&LockfileEntry>,
    ) -> anyhow::Result<Option<AssetRef>>;
}

pub struct Params {
    pub api_key: Option<String>,
    pub creator: config::Creator,
    pub expected_price: Option<u32>,
    pub project_dir: PathBuf,
}
