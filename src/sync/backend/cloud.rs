use super::{SyncBackend, SyncResult};
use crate::{
    asset::{Asset, AssetKind, ModelKind},
    sync::SyncState,
    upload::{upload_animation, upload_cloud},
};
use anyhow::bail;

pub struct CloudBackend;

impl SyncBackend for CloudBackend {
    async fn sync(
        &self,
        state: &mut SyncState,
        asset: &Asset,
    ) -> anyhow::Result<Option<SyncResult>> {
        let asset_id = match asset.kind {
            AssetKind::Decal(_) | AssetKind::Audio(_) | AssetKind::Model(ModelKind::Model) => {
                upload_cloud(asset, state.api_key.clone(), &state.config.creator).await?
            }
            AssetKind::Model(ModelKind::Animation(_)) => {
                let Some(cookie) = state.cookie.clone() else {
                    bail!("Cookie required for uploading animations")
                };

                let res =
                    upload_animation(asset, cookie, state.csrf.clone(), &state.config.creator)
                        .await?;

                state.csrf = Some(res.csrf);
                res.asset_id
            }
        };

        Ok(Some(SyncResult::Cloud(asset_id)))
    }
}
