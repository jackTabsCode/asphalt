use super::{
    backend::{cloud::CloudBackend, debug::DebugBackend, studio::StudioBackend, SyncBackend},
    SyncState,
};
use crate::{asset::Asset, cli::SyncTarget, config::Input, sync::SyncResult};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, warn};
use std::sync::Arc;

pub async fn perform(
    state: Arc<SyncState>,
    input: &Input,
    assets: &Vec<Asset>,
) -> anyhow::Result<()> {
    let backend = pick_backend(&state.args.target.clone()).await?;

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

    for asset in assets {
        let display = asset.path.display();
        debug!("Syncing asset {}", display);

        progress_bar.set_message(format!("Syncing \"{}\"", display));
        progress_bar.inc(1);

        let res = match backend {
            TargetBackend::Debug(ref backend) => backend.sync(state.clone(), input, asset).await,
            TargetBackend::Cloud(ref backend) => backend.sync(state.clone(), input, asset).await,
            TargetBackend::Studio(ref backend) => backend.sync(state.clone(), input, asset).await,
        };

        progress_bar.set_message(format!("Writing {}", display));

        match res {
            Ok(Some(result)) => {
                state
                    .result_tx
                    .send(SyncResult {
                        input: input.clone(),
                        hash: asset.hash.clone(),
                        path: asset.path.clone(),
                        backend: result,
                    })
                    .await?;
            }
            Err(err) => {
                warn!("Failed to sync asset {}: {}", display, err);
            }
            _ => {}
        };
    }

    Ok(())
}

enum TargetBackend {
    Debug(DebugBackend),
    Cloud(CloudBackend),
    Studio(StudioBackend),
}

async fn pick_backend(target: &SyncTarget) -> anyhow::Result<TargetBackend> {
    match target {
        SyncTarget::Debug => Ok(TargetBackend::Debug(DebugBackend::new().await?)),
        SyncTarget::Cloud => Ok(TargetBackend::Cloud(CloudBackend::new().await?)),
        SyncTarget::Studio => Ok(TargetBackend::Studio(StudioBackend::new().await?)),
    }
}
