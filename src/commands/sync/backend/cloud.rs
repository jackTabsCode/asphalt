use anyhow::Context;
use log::info;

use crate::{
    asset::{Asset, AssetKind, ModelKind},
    commands::sync::state::SyncState,
    upload::{upload_animation, upload_cloud_asset},
};

use super::{SyncBackend, SyncResult};

pub struct CloudBackend;

impl SyncBackend for CloudBackend {
    async fn sync(
        &self,
        state: &mut SyncState,
        path: &str,
        asset: Asset,
    ) -> anyhow::Result<SyncResult> {
        let asset_id = match asset.kind() {
            AssetKind::Decal(_) | AssetKind::Audio(_) | AssetKind::Model(ModelKind::Model) => {
                let cloud_type = asset
                    .cloud_type()
                    .ok_or_else(|| anyhow::anyhow!("Invalid cloud type"))?;

                upload_cloud_asset(
                    asset.data().to_owned(),
                    asset.name().to_owned(),
                    cloud_type,
                    state.api_key.to_owned(),
                    state.creator.to_owned(),
                )
                .await
            }
            AssetKind::Model(ModelKind::Animation) => {
                if let Some(cookie) = state.cookie.to_owned() {
                    let result = upload_animation(
                        asset.data().to_owned(),
                        asset.name().to_owned(),
                        cookie,
                        state.csrf.to_owned(),
                        state.creator.to_owned(),
                    )
                    .await?;
                    state.update_csrf(Some(result.csrf));
                    Ok(result.asset_id)
                } else {
                    Err(anyhow::anyhow!("Cookie required for uploading animations"))
                }
            }
        }
        .with_context(|| format!("Failed to upload {path}"))?;

        info!("Uploaded {path}");
        Ok(SyncResult::Cloud(asset_id))
    }
}
