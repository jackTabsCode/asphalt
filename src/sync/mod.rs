use crate::{
    asset::Asset,
    auth::Auth,
    cli::{SyncArgs, SyncTarget},
    config::{Codegen, Config, Input},
    lockfile::{Lockfile, LockfileEntry, RawLockfile},
    web_api::WebApiClient,
};
use anyhow::{Context, Result, bail};
use backend::BackendSyncResult;
use codegen::{CodegenInput, CodegenLanguage, CodegenNode};
use indicatif::MultiProgress;
use log::{debug, warn};
use resvg::usvg::fontdb::Database;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{fs, sync::mpsc, task::JoinHandle};
use walk::{DuplicateResult, WalkResult};

mod backend;
mod codegen;
mod perform;
mod process;
mod walk;

pub struct SyncResult {
    hash: String,
    path: PathBuf,
    input_name: String,
    backend: BackendSyncResult,
}

pub struct SyncState {
    args: SyncArgs,

    existing_lockfile: Lockfile,
    result_tx: mpsc::Sender<SyncResult>,

    multi_progress: MultiProgress,

    font_db: Arc<Database>,

    client: WebApiClient,
}

struct CodegenInsertion {
    input_name: String,
    asset_path: PathBuf,
    asset_id: String,
}

struct LockfileInsertion {
    input_name: String,
    hash: String,
    entry: LockfileEntry,
    write: bool,
}

struct DuplicateInsertion {
    input_name: String,
    inner: DuplicateResult,
}

pub async fn sync(multi_progress: MultiProgress, args: SyncArgs) -> Result<()> {
    if args.dry_run && !matches!(args.target, SyncTarget::Cloud) {
        bail!("A dry run doesn't make sense in this context");
    }

    let config = Config::read().await?;
    let codegen_config = config.codegen.clone();

    let lockfile = RawLockfile::read().await?.into_lockfile()?;

    let key_required = matches!(args.target, SyncTarget::Cloud) && !args.dry_run;
    let auth = Auth::new(args.api_key.clone(), key_required)?;

    let font_db = Arc::new({
        let mut db = Database::new();
        db.load_system_fonts();
        db
    });

    let (result_tx, mut result_rx) = mpsc::channel::<SyncResult>(100);
    let (lockfile_tx, mut lockfile_rx) = mpsc::channel::<LockfileInsertion>(100);
    let (codegen_tx, mut codegen_rx) = mpsc::channel::<CodegenInsertion>(100);

    let state = Arc::new(SyncState {
        args: args.clone(),

        existing_lockfile: lockfile,
        result_tx,

        multi_progress,

        font_db,

        client: WebApiClient::new(auth, config.creator, args.expected_price),
    });

    let mut codegen_inputs: HashMap<String, CodegenInput> = HashMap::new();
    for (input_name, input) in &config.inputs.clone() {
        for (path, asset) in &input.web {
            let entry = codegen_inputs.entry(input_name.clone()).or_default();
            let path = path.replace('\\', "/");

            entry.insert(PathBuf::from(path), format!("rbxassetid://{}", asset.id));
        }
    }

    let mut consumer_handles = Vec::<JoinHandle<Result<()>>>::new();

    let lockfile_handle = tokio::spawn(async move {
        let mut new_lockfile = Lockfile::default();

        while let Some(insertion) = lockfile_rx.recv().await {
            if matches!(args.target, SyncTarget::Cloud) {
                new_lockfile.insert(&insertion.input_name, &insertion.hash, insertion.entry);
                if insertion.write {
                    new_lockfile.write(None).await?;
                }
            }
        }

        Ok::<_, anyhow::Error>(new_lockfile)
    });

    let lockfile_tx_backend = lockfile_tx.clone();
    let codegen_tx_backend = codegen_tx.clone();

    consumer_handles.push(tokio::spawn(async move {
        while let Some(result) = result_rx.recv().await {
            if let BackendSyncResult::Cloud(asset_id) = result.backend {
                lockfile_tx_backend
                    .send(LockfileInsertion {
                        input_name: result.input_name.clone(),
                        hash: result.hash,
                        entry: LockfileEntry { asset_id },
                        write: true,
                    })
                    .await?;

                codegen_tx_backend
                    .send(CodegenInsertion {
                        input_name: result.input_name,
                        asset_path: result.path,
                        asset_id: format!("rbxassetid://{asset_id}"),
                    })
                    .await?;
            } else if let BackendSyncResult::Studio(asset_id) = result.backend {
                codegen_tx_backend
                    .send(CodegenInsertion {
                        input_name: result.input_name,
                        asset_path: result.path.clone(),
                        asset_id,
                    })
                    .await?;
            }
        }

        Ok(())
    }));

    let inputs = config.inputs.clone();

    let codegen_handle = tokio::spawn(async move {
        while let Some(insertion) = codegen_rx.recv().await {
            let codegen_input = codegen_inputs
                .entry(insertion.input_name.clone())
                .or_default();
            let input = inputs
                .get(&insertion.input_name)
                .context("Failed to find input for codegen input")?;

            let path = insertion
                .asset_path
                .strip_prefix(input.path.get_prefix())
                .unwrap();

            let path = path.to_string_lossy().replace('\\', "/");

            codegen_input.insert(path.into(), insertion.asset_id);
        }

        Ok::<_, anyhow::Error>(codegen_inputs)
    });

    let mut producer_handles = Vec::<JoinHandle<Result<()>>>::new();

    let (duplicate_tx, mut duplicate_rx) = mpsc::channel::<DuplicateInsertion>(100);

    let inputs = config.inputs.clone();

    let duplicate_handle = tokio::spawn(async move {
        let mut duplicate_assets = HashMap::<String, Vec<DuplicateResult>>::new();

        while let Some(insertion) = duplicate_rx.recv().await {
            let input = inputs
                .get(&insertion.input_name)
                .context("Failed to find input for codegen input")?;

            let original_path = insertion
                .inner
                .original_path
                .strip_prefix(input.path.get_prefix())
                .unwrap()
                .to_owned();

            let path = insertion
                .inner
                .path
                .strip_prefix(input.path.get_prefix())
                .unwrap()
                .to_owned();

            duplicate_assets
                .entry(insertion.input_name)
                .or_default()
                .push(DuplicateResult {
                    original_path,
                    path,
                });
        }

        Ok::<_, anyhow::Error>(duplicate_assets)
    });

    for (input_name, input) in &config.inputs {
        let state = state.clone();
        let input = input.clone();
        let lockfile_tx = lockfile_tx.clone();
        let codegen_tx = codegen_tx.clone();
        let input_name = input_name.clone();
        let duplicate_tx = duplicate_tx.clone();

        producer_handles.push(tokio::spawn(async move {
            debug!("Walking input {input_name}");
            let walk_results = walk::walk(state.clone(), input_name.clone(), &input).await?;

            let mut new_assets = Vec::<Asset>::new();
            let mut not_new_count = 0;
            let mut dupe_count = 0;

            for result in walk_results {
                match result {
                    WalkResult::New(asset) => {
                        new_assets.push(asset);
                    }
                    WalkResult::Existing(existing) => {
                        not_new_count += 1;

                        lockfile_tx
                            .send(LockfileInsertion {
                                input_name: input_name.clone(),
                                hash: existing.hash,
                                entry: existing.entry.clone(),
                                // This takes too long, and we're not really losing anything here.
                                write: false,
                            })
                            .await?;

                        codegen_tx
                            .send(CodegenInsertion {
                                input_name: input_name.clone(),
                                asset_path: existing.path.clone(),
                                asset_id: format!("rbxassetid://{}", existing.entry.asset_id),
                            })
                            .await?;
                    }
                    WalkResult::Duplicate(dupe) => {
                        if input.warn_each_duplicate {
                            warn!(
                                "Duplicate file found: {} (original at {})",
                                dupe.path.display(),
                                dupe.original_path.display()
                            );
                        }

                        dupe_count += 1;

                        duplicate_tx
                            .send(DuplicateInsertion {
                                input_name: input_name.clone(),
                                inner: dupe,
                            })
                            .await?;
                    }
                }
            }

            if dupe_count > 0 {
                warn!(
                    "{dupe_count} duplicate files found. Code will still be generated for them.",
                );
            }

            debug!(
                "Discovered {} unchanged files and {} new or\
                changed files for input {}, starting processing",
                not_new_count,
                new_assets.len(),
                input_name
            );

            process::process(state.clone(), input_name.clone(), &input, &mut new_assets).await?;
            perform::perform(state, input_name.clone(), &input, &new_assets).await?;

            Ok(())
        }));
    }

    for handle in producer_handles {
        handle.await??;
    }

    drop(state);
    drop(lockfile_tx);
    drop(codegen_tx);
    drop(duplicate_tx);

    for handle in consumer_handles {
        handle.await??;
    }

    let new_lockfile = lockfile_handle.await??;
    if matches!(args.target, SyncTarget::Cloud) {
        new_lockfile.write(None).await?;
    }

    let mut codegen_inputs = codegen_handle.await??;
    let duplicate_inputs = duplicate_handle.await??;

    for (input_name, dupes) in duplicate_inputs {
        let input = codegen_inputs.get_mut(&input_name).unwrap();
        for dupe in dupes {
            let original = input.get(&dupe.original_path).unwrap();
            input.insert(dupe.path, original.clone());
        }
    }

    for (input_name, codegen_input) in codegen_inputs {
        let input = config
            .inputs
            .get(&input_name)
            .context("Failed to find input for codegen input")?;

        generate_from_input(
            &input_name,
            input,
            &codegen_config,
            &codegen_input,
            CodegenLanguage::Luau,
        )
        .await?;
        if codegen_config.typescript {
            generate_from_input(
                &input_name,
                input,
                &codegen_config,
                &codegen_input,
                CodegenLanguage::TypeScript,
            )
            .await?;
        }
    }

    Ok(())
}

async fn generate_from_input(
    input_name: &str,
    input: &Input,
    style: &Codegen,
    codegen_input: &CodegenInput,
    lang: CodegenLanguage,
) -> anyhow::Result<()> {
    let node: CodegenNode = codegen::from_codegen_input(codegen_input, style);
    let ext = match lang {
        CodegenLanguage::Luau => "luau",
        CodegenLanguage::TypeScript => "d.ts",
    };
    let code = codegen::generate_code(lang, input_name, &node)?;

    fs::create_dir_all(&input.output_path).await?;
    fs::write(input.output_path.join(format!("{input_name}.{ext}")), code).await?;

    Ok(())
}
