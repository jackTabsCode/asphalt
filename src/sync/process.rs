use super::SyncState;
use crate::{asset::Asset, config::Input, progress_bar::ProgressBar};
use anyhow::bail;
use log::{debug, info, warn};
use std::sync::Arc;

pub async fn process(
    state: Arc<SyncState>,
    input_name: String,
    input: &Input,
    assets: &mut Vec<Asset>,
) -> anyhow::Result<()> {
    let prefix = if state.args.dry_run {
        "Checking"
    } else {
        "Processing"
    };
    let pb = ProgressBar::new(
        state.multi_progress.clone(),
        &format!("{prefix} input \"{input_name}\""),
        assets.len(),
    );

    let mut dry_run_count = 0;

    for asset in assets {
        let file_name = asset.path.display().to_string();
        pb.set_msg(&file_name);

        if state.args.dry_run {
            info!("File {file_name} would be synced");
            dry_run_count += 1;

            continue;
        } else {
            debug!("File {file_name} changed, syncing");
        }

        if let Err(err) = asset.process(state.font_db.clone(), input.bleed).await {
            warn!("Skipping file {file_name} because it failed processing: {err:?}");
            continue;
        }

        pb.inc(1);
    }

    pb.finish();

    if state.args.dry_run && dry_run_count > 0 {
        bail!("{} files would be synced", dry_run_count);
    }

    Ok(())
}
