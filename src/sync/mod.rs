use crate::{
    asset::Asset,
    auth::Auth,
    cli::{SyncArgs, SyncTarget},
    config::{Config, Input},
    lockfile::{Lockfile, LockfileEntry},
};
use anyhow::bail;
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

pub async fn sync(multi_progress: MultiProgress, args: SyncArgs) -> anyhow::Result<()> {
    if args.dry_run && !matches!(&args.target, Some(SyncTarget::Cloud)) {
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

    let mut new_lockfile = Lockfile::default();

    struct CodegenInsertion {
        output_path: PathBuf,
        asset_path: PathBuf,
        asset_id: String,
    }

    let mut codegen_inputs: HashMap<PathBuf, CodegenInput> = HashMap::new();

    let (codegen_tx, mut codegen_rx) = mpsc::channel::<CodegenInsertion>(100);

    let (result_tx, mut result_rx) = mpsc::channel::<SyncResult>(100);

    let csrf = Arc::new(RwLock::new(None));

    let state = Arc::new(SyncState {
        client: reqwest::Client::new(),
        args: args.clone(),
        multi_progress,
        config: config.clone(),
        existing_lockfile: lockfile.clone(),
        result_tx: result_tx.clone(),
        font_db,
        auth,
        csrf: csrf.clone(),
    });

    type Handle = JoinHandle<anyhow::Result<()>>;

    struct LockfileInsertion {
        input: Input,
        path: PathBuf,
        entry: LockfileEntry,
    }

    let (lockfile_tx, mut lockfile_rx) = mpsc::channel::<LockfileInsertion>(100);

    let mut consumer_handles = Vec::<Handle>::new();

    if matches!(args.target, Some(SyncTarget::Cloud)) {
        consumer_handles.push(tokio::spawn(async move {
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
        while let Some(tx) = result_rx.recv().await {
            if let BackendSyncResult::Cloud(asset_id) = tx.backend {
                lockfile_tx_backend
                    .send(LockfileInsertion {
                        input: tx.input.clone(),
                        path: tx.path.clone(),
                        entry: LockfileEntry {
                            hash: tx.hash,
                            asset_id,
                        },
                    })
                    .await?;

                codegen_tx_backend
                    .send(CodegenInsertion {
                        output_path: tx.input.output_path,
                        asset_path: tx.path,
                        asset_id: format!("rbxassetid://{}", asset_id),
                    })
                    .await?;
            }
        }

        Ok(())
    }));

    let mut producer_handles: Vec<Handle> = Vec::new();

    for input in config.inputs.clone() {
        let state = state.clone();

        let lockfile_tx_walk = lockfile_tx.clone();
        let codegen_tx = codegen_tx.clone();

        producer_handles.push(tokio::spawn(async move {
            debug!("Walking input {}", input.name);
            let walk_res = walk::walk(state.clone(), &input).await?;

            let mut new_assets = Vec::<Asset>::new();
            let mut not_new_assets = 0;

            for res in walk_res {
                match res {
                    WalkFileResult::NewAsset(asset) => {
                        new_assets.push(asset);
                    }
                    WalkFileResult::ExistingAsset(entry) => {
                        not_new_assets += 1;

                        lockfile_tx_walk
                            .send(LockfileInsertion {
                                input: input.clone(),
                                path: entry.0.clone(),
                                entry: entry.1.clone(),
                            })
                            .await?;

                        codegen_tx
                            .send(CodegenInsertion {
                                output_path: input.output_path.clone(),
                                asset_path: entry.0,
                                asset_id: format!("rbxassetid://{}", entry.1.asset_id),
                            })
                            .await?;
                    }
                }
            }

            debug!(
                "Discovered {} unchanged files and {} new or changed\
                files for input {}, starting processing",
                not_new_assets,
                new_assets.len(),
                input.name
            );
            process::process(state.clone(), &input, &mut new_assets).await?;

            debug!("Starting perform for input {}", input.name);
            perform::perform(state, &input, &new_assets).await?;

            Ok(())
        }));
    }

    for handle in producer_handles {
        handle.await??;
    }

    drop(state);

    drop(lockfile_tx);
    drop(result_tx);
    drop(codegen_tx);

    for handle in consumer_handles {
        handle.await??;
    }

    while let Some(insertion) = codegen_rx.recv().await {
        let input = codegen_inputs
            .entry(insertion.output_path.clone())
            .or_default();

        input.insert(insertion.asset_path, insertion.asset_id);
    }

    dbg!(codegen_inputs);

    Ok(())
}
