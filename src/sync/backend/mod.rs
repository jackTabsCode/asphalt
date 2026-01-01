use std::sync::Arc;

use super::State;
use crate::{
    asset::{Asset, AssetRef},
    config,
};

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
        state: Arc<State>,
        input_name: String,
        asset: &Asset,
    ) -> anyhow::Result<Option<AssetRef>>;
}

pub struct Params {
    pub api_key: Option<String>,
    pub creator: config::Creator,
    pub expected_price: Option<u32>,
}
