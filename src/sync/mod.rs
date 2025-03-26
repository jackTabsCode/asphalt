use crate::{
    asset::Asset,
    auth::Auth,
    cli::{SyncArgs, SyncTarget},
    config::{Config, Input},
    lockfile::{Lockfile, LockfileEntry},
};
use anyhow::{bail, Result};
use backend::BackendSyncResult;
use codegen::CodegenInput;
use indicatif::MultiProgress;
use log::debug;
use resvg::usvg::fontdb::Database;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};
use walk::WalkFileResult;

mod backend;
mod codegen;
mod perform;
mod process;
mod walk;

pub struct SyncResult {
    hash: String,
    path: PathBuf,
    input: Input,
    backend: BackendSyncResult,
}

pub struct SyncState {
    client: reqwest::Client,
    args: SyncArgs,
    config: Config,
    existing_lockfile: Lockfile,
    result_tx: mpsc::Sender<SyncResult>,
    multi_progress: MultiProgress,
    font_db: Arc<Database>,
    auth: Auth,
    csrf: Arc<RwLock<Option<String>>>,
}

struct CodegenInsertion {
    output_path: PathBuf,
    asset_path: PathBuf,
    asset_id: String,
}

struct LockfileInsertion {
    input: Input,
    path: PathBuf,
    entry: LockfileEntry,
}

pub async fn sync(multi_progress: MultiProgress, args: SyncArgs) -> Result<()> {
    if args.dry_run && !matches!(&args.target, SyncTarget::Cloud) {
        bail!("A dry run doesn't make sense in this context");
    }

    let config = Config::read()?;
    let lockfile = Lockfile::read().await?;
    let auth = Auth::new(args.api_key.clone())?;

    let font_db = Arc::new({
        let mut db = Database::new();
        db.load_system_fonts();
        db
    });

    let (result_tx, mut result_rx) = mpsc::channel::<SyncResult>(100);
    let (lockfile_tx, mut lockfile_rx) = mpsc::channel::<LockfileInsertion>(100);
    let (codegen_tx, mut codegen_rx) = mpsc::channel::<CodegenInsertion>(100);

    let state = Arc::new(SyncState {
        client: reqwest::Client::new(),
        args: args.clone(),
        multi_progress,
        config: config.clone(),
        existing_lockfile: lockfile,
        result_tx,
        font_db,
        auth,
        csrf: Arc::new(RwLock::new(None)),
    });

    let mut codegen_inputs: HashMap<PathBuf, CodegenInput> = HashMap::new();
    for input in &config.inputs {
        for (path, asset) in &input.web_assets {
            codegen_inputs
                .entry(input.output_path.clone())
                .or_default()
                .insert(PathBuf::from(path), format!("rbxassetid://{}", asset.id));
        }
    }

    let mut consumer_handles = Vec::<JoinHandle<Result<()>>>::new();

    if matches!(args.target, SyncTarget::Cloud) {
        consumer_handles.push(tokio::spawn(async move {
            let mut new_lockfile = Lockfile::default();

            while let Some(insertion) = lockfile_rx.recv().await {
                new_lockfile.insert(insertion.input.name, &insertion.path, insertion.entry);
                new_lockfile.write(None).await?;
            }

            Ok(())
        }));
    }

    let lockfile_tx_backend = lockfile_tx.clone();
    let codegen_tx_backend = codegen_tx.clone();

    consumer_handles.push(tokio::spawn(async move {
        while let Some(result) = result_rx.recv().await {
            if let BackendSyncResult::Cloud(asset_id) = result.backend {
                lockfile_tx_backend
                    .send(LockfileInsertion {
                        input: result.input.clone(),
                        path: result.path.clone(),
                        entry: LockfileEntry {
                            hash: result.hash,
                            asset_id,
                        },
                    })
                    .await?;

                codegen_tx_backend
                    .send(CodegenInsertion {
                        output_path: result.input.output_path,
                        asset_path: result.path,
                        asset_id: format!("rbxassetid://{}", asset_id),
                    })
                    .await?;
            } else if let BackendSyncResult::Studio(asset_id) = result.backend {
                codegen_tx_backend
                    .send(CodegenInsertion {
                        output_path: result.input.output_path.clone(),
                        asset_path: result.path.clone(),
                        asset_id,
                    })
                    .await?;
            }
        }

        Ok(())
    }));

    let mut producer_handles = Vec::<JoinHandle<Result<()>>>::new();

    for input in &config.inputs {
        let state = state.clone();
        let input = input.clone();
        let lockfile_tx = lockfile_tx.clone();
        let codegen_tx = codegen_tx.clone();

        producer_handles.push(tokio::spawn(async move {
            debug!("Walking input {}", input.name);
            let walk_results = walk::walk(state.clone(), &input).await?;

            let mut new_assets = Vec::<Asset>::new();
            let mut not_new_count = 0;

            for result in walk_results {
                match result {
                    WalkFileResult::NewAsset(asset) => {
                        new_assets.push(asset);
                    }
                    WalkFileResult::ExistingAsset((path, entry)) => {
                        not_new_count += 1;

                        lockfile_tx
                            .send(LockfileInsertion {
                                input: input.clone(),
                                path: path.clone(),
                                entry: entry.clone(),
                            })
                            .await?;

                        codegen_tx
                            .send(CodegenInsertion {
                                output_path: input.output_path.clone(),
                                asset_path: path.clone(),
                                asset_id: format!("rbxassetid://{}", entry.asset_id),
                            })
                            .await?;
                    }
                }
            }

            debug!(
                "Discovered {} unchanged files and {} new or\
                changed files for input {}, starting processing",
                not_new_count,
                new_assets.len(),
                input.name
            );

            process::process(state.clone(), &input, &mut new_assets).await?;
            perform::perform(state, &input, &new_assets).await?;

            Ok(())
        }));
    }

    for handle in producer_handles {
        handle.await??;
    }

    drop(state);
    drop(lockfile_tx);
    drop(codegen_tx);

    for handle in consumer_handles {
        handle.await??;
    }

    while let Some(insertion) = codegen_rx.recv().await {
        let input = codegen_inputs.entry(insertion.output_path).or_default();

        input.insert(insertion.asset_path, insertion.asset_id);
    }

    dbg!(codegen_inputs);

    Ok(())
}
