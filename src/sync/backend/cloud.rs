use super::Backend;
use crate::{
    asset::{Asset, AssetRef},
    sync::{State, backend::Params},
    web_api::WebApiClient,
};
use anyhow::{Context, bail};
use std::sync::Arc;

pub struct Cloud {
    client: WebApiClient,
}

impl Backend for Cloud {
    async fn new(params: Params) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            client: WebApiClient::new(
                params
                    .api_key
                    .context("An API key is required to use the Cloud backend")?,
                params.creator,
                params.expected_price,
            ),
        })
    }

    async fn sync(
        &self,
        _: Arc<State>,
        _: String,
        asset: &Asset,
    ) -> anyhow::Result<Option<AssetRef>> {
        match self.client.upload(asset).await {
            Ok(id) => Ok(Some(AssetRef::Cloud(id))),
            Err(err) => bail!("Failed to upload asset: {err:?}"),
        }
    }
}
