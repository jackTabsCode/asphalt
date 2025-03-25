use super::SyncState;
use crate::{asset::Asset, config::Input};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info, warn};
use std::sync::Arc;

pub async fn process_input(
    state: Arc<SyncState>,
    input: &Input,
    assets: Vec<Asset>,
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

    for mut asset in assets {
        let display = asset.path.display().to_string();

        let message = format!("Processing \"{}\"", display);
        progress_bar.set_message(message);
        progress_bar.inc(1);

        if state.args.dry_run {
            info!("File {} would be synced", display);
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

    Ok(())
}
