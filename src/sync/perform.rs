use super::{
    SyncState,
    backend::{SyncBackend, cloud::CloudBackend, debug::DebugBackend, studio::StudioBackend},
};
use crate::{
    asset::Asset, cli::SyncTarget, config::Input, progress_bar::ProgressBar, sync::SyncResult,
};
use log::warn;
use std::sync::Arc;

pub async fn perform(
    state: Arc<SyncState>,
    input_name: String,
    input: &Input,
    assets: &Vec<Asset>,
) -> anyhow::Result<()> {
    let backend = pick_backend(&state.args.target.clone()).await?;

    let pb = ProgressBar::new(
        state.multi_progress.clone(),
        &format!("Syncing input \"{input_name}\""),
        assets.len(),
    );

    for asset in assets {
        let input_name = input_name.clone();

        let file_name = asset.path.display().to_string();
        pb.set_msg(&file_name);

        let res = match backend {
            TargetBackend::Debug(ref backend) => {
                backend
                    .sync(state.clone(), input_name.clone(), input, asset)
                    .await
            }
            TargetBackend::Cloud(ref backend) => {
                backend
                    .sync(state.clone(), input_name.clone(), input, asset)
                    .await
            }
            TargetBackend::Studio(ref backend) => {
                backend
                    .sync(state.clone(), input_name.clone(), input, asset)
                    .await
            }
        };

        match res {
            Ok(Some(result)) => {
                state
                    .result_tx
                    .send(SyncResult {
                        input_name: input_name.clone(),
                        hash: asset.hash.clone(),
                        path: asset.path.clone(),
                        backend: result,
                    })
                    .await?;
            }
            Err(err) => {
                warn!("Failed to sync asset {file_name}: {err:?}");
            }
            _ => {}
        };

        pb.inc(1);
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
