use super::{SyncBackend, SyncResult};
use crate::{
    asset::{Asset, AssetKind, ModelKind},
    sync::SyncState,
    upload::upload_cloud,
};

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
            _ => todo!(),
        };

        Ok(Some(SyncResult::Cloud(asset_id)))
    }
}
