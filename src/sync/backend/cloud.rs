use super::Backend;
use crate::{
    asset::{Asset, AssetRef},
    lockfile::LockfileEntry,
    sync::backend::Params,
    web_api::WebApiClient,
};
use anyhow::{Context, bail};

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
        asset: &Asset,
        lockfile_entry: Option<&LockfileEntry>,
    ) -> anyhow::Result<Option<AssetRef>> {
        if let Some(lockfile_entry) = lockfile_entry {
            return Ok(Some(lockfile_entry.into()));
        }

        match self.client.upload(asset).await {
            Ok(id) => Ok(Some(AssetRef::Cloud(id))),
            Err(err) => bail!("Failed to upload asset: {err:?}"),
        }
    }
}
