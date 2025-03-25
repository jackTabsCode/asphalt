use super::SyncState;
use crate::{asset::Asset, config::Input};
use anyhow::bail;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info, warn};
use std::sync::Arc;

pub async fn process(
    state: Arc<SyncState>,
    input: &Input,
    assets: &mut Vec<Asset>,
) -> anyhow::Result<()> {
    let progress_bar = state.multi_progress.add(
        ProgressBar::new(assets.len() as u64)
            .with_prefix(input.name.clone())
            .with_style(
                ProgressStyle::default_bar()
                    .template("Input \"{prefix}\"\n {msg}\n Progress: {pos}/{len} | ETA: {eta}\n[{bar:40.cyan/blue}]")
                    .unwrap()
                    .progress_chars("=> "),
            ),
    );

    let mut dry_run_count = 0;

    for asset in assets {
        let display = asset.path.display().to_string();

        let message = format!(
            "{} \"{}\"",
            if state.args.dry_run {
                "Checking"
            } else {
                "Processing"
            },
            display
        );
        progress_bar.set_message(message);
        progress_bar.inc(1);

        if state.args.dry_run {
            info!("File {} would be synced", display);
            dry_run_count += 1;
            continue;
        } else {
            debug!("File {} changed, syncing", display);
        }

        if let Err(err) = asset.process(state.font_db.clone(), input.bleed).await {
            warn!(
                "Skipping file {} because it failed processing: {}",
                display, err
            );
            continue;
        }
    }

    if state.args.dry_run && dry_run_count > 0 {
        bail!("{} files would be synced", dry_run_count);
    }

    Ok(())
}
