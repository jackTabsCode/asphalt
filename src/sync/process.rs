use super::SyncState;
use crate::{asset::Asset, progress_bar::ProgressBar};
use log::warn;
use std::sync::Arc;

pub async fn process(
    assets: Vec<Asset>,
    state: Arc<SyncState>,
    input_name: String,
    bleed: bool,
) -> anyhow::Result<Vec<Asset>> {
    let pb = ProgressBar::new(
        state.multi_progress.clone(),
        &format!("Processing input \"{input_name}\""),
        assets.len(),
    );

    let mut processed_assets = Vec::with_capacity(assets.len());

    for mut asset in assets {
        let file_name = asset.path.to_string();
        pb.set_msg(&file_name);

        if let Err(err) = asset.process(state.font_db.clone(), bleed).await {
            warn!("Skipping file {file_name} because it failed processing: {err:?}");
            continue;
        }

        pb.inc(1);

        processed_assets.push(asset);
    }

    pb.finish();

    Ok(processed_assets)
}
