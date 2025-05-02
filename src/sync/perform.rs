use super::{
    SyncState,
    backend::{SyncBackend, cloud::CloudBackend, debug::DebugBackend, studio::StudioBackend},
};
use crate::{asset::Asset, cli::SyncTarget, config::Input, sync::SyncResult};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, warn};
use std::{collections::HashSet, sync::Arc};
use tokio::{sync::mpsc, task};

pub async fn perform(
    state: Arc<SyncState>,
    input_name: String,
    input: &Input,
    assets: &Vec<Asset>,
) -> anyhow::Result<()> {
    let backend = pick_backend(&state.args.target.clone()).await?;

    let (status_tx, mut status_rx) = mpsc::channel::<(String, bool)>(100);

    {
        let state = state.clone();
        let input_name = input_name.clone();
        let num_assets = assets.len() as u64;

        task::spawn(async move {
            let mut active = HashSet::new();

            let progress_bar = state.multi_progress.add(
            ProgressBar::new(num_assets)
                .with_prefix(input_name.clone())
                .with_style(
                    ProgressStyle::default_bar()
                        .template("Input \"{prefix}\"\n {msg}\n Progress: {pos}/{len} | ETA: {eta}\n[{bar:40.cyan/blue}]")
                        .unwrap()
                        .progress_chars("=> "),
                ),
        );

            while let Some((id, finished)) = status_rx.recv().await {
                if finished {
                    active.remove(&id);
                    progress_bar.inc(1);
                } else {
                    active.insert(id);
                }

                let str = active
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");

                progress_bar.set_message(format!("Syncing {str}"));
            }
        });
    }

    let mut tasks = FuturesUnordered::new();

    for asset in assets {
        let input_name = input_name.clone();
        let state = state.clone();
        let input = input.clone();
        let backend = backend.clone();
        let status_tx = status_tx.clone();

        tasks.push(async move {
            let display = asset.path.display().to_string();
            debug!("Syncing asset {}", display);

            let _ = status_tx.send((display.clone(), false)).await;

            let res = match backend {
                TargetBackend::Debug(ref backend) => {
                    backend
                        .sync(state.clone(), input_name.clone(), &input, asset)
                        .await
                }
                TargetBackend::Cloud(ref backend) => {
                    backend
                        .sync(state.clone(), input_name.clone(), &input, asset)
                        .await
                }
                TargetBackend::Studio(ref backend) => {
                    backend
                        .sync(state.clone(), input_name.clone(), &input, asset)
                        .await
                }
            };

            match res {
                Ok(Some(result)) => {
                    if let Err(err) = state
                        .result_tx
                        .send(SyncResult {
                            input_name: input_name.clone(),
                            hash: asset.hash.clone(),
                            path: asset.path.clone(),
                            backend: result,
                        })
                        .await
                    {
                        warn!("Failed to send sync result: {:?}", err);
                    }
                }
                Err(err) => {
                    warn!("Failed to sync asset {}: {:?}", display, err);
                }
                _ => {}
            };

            let _ = status_tx.send((display.clone(), true)).await;
        });
    }

    while tasks.next().await.is_some() {}
    drop(status_tx);

    Ok(())
}

#[derive(Clone)]
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
