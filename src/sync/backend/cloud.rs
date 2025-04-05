use super::{BackendSyncResult, SyncBackend};
use crate::{
    asset::{Asset, AssetKind, ModelKind},
    config::Input,
    sync::SyncState,
    upload::{upload_animation, upload_cloud},
};
use anyhow::bail;
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
        _input: &Input,
        asset: &Asset,
    ) -> anyhow::Result<Option<BackendSyncResult>> {
        if cfg!(feature = "mock_cloud") {
            time::sleep(time::Duration::from_secs(1)).await;
            return Ok(Some(BackendSyncResult::Cloud(1337)));
        }

        let asset_id = match asset.kind {
            AssetKind::Decal(_) | AssetKind::Audio(_) | AssetKind::Model(ModelKind::Model) => {
                upload_cloud(
                    state.client.clone(),
                    asset,
                    state.auth.cookie.clone().unwrap(),
                    state.csrf.read().await.as_ref().cloned(),
                    state.auth.api_key.clone(),
                    &state.config.creator,
                )
                .await?
            }
            AssetKind::Model(ModelKind::Animation(_)) => {
                let Some(cookie) = state.auth.cookie.clone() else {
                    bail!("Cookie required for uploading animations")
                };

                let res = upload_animation(
                    state.client.clone(),
                    asset,
                    cookie,
                    Some(state.csrf.read().await.as_ref().unwrap().clone()),
                    &state.config.creator,
                )
                .await?;

                *state.csrf.write().await = Some(res.csrf);

                res.asset_id
            }
        };

        Ok(Some(BackendSyncResult::Cloud(asset_id)))
    }
}
