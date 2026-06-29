use crate::{
    asset::{Asset, AssetRef},
    cli::{SyncArgs, SyncTarget},
    config::{Config, Input, PackOptions},
    hash::Hash,
    lockfile::{LockfileEntry, RawLockfile, SpriteInfo},
    pack::{self, Packer},
    sync::{backend::Backend, collect::collect_events},
};
use anyhow::{Context, bail};
use fs_err::tokio as fs;
use indicatif::MultiProgress;
use log::info;
use relative_path::RelativePathBuf;
use resvg::usvg::fontdb;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::mpsc;

mod backend;
mod codegen;
mod collect;
mod walk;

enum TargetBackend {
    Cloud(backend::Cloud),
    Debug(backend::Debug),
    Studio(backend::Studio),
}

impl TargetBackend {
    pub async fn sync(
        &self,
        asset: &Asset,
        lockfile_entry: Option<&LockfileEntry>,
    ) -> anyhow::Result<Option<AssetRef>> {
        match self {
            Self::Cloud(cloud_backend) => cloud_backend.sync(asset, lockfile_entry).await,
            Self::Debug(debug_backend) => debug_backend.sync(asset, lockfile_entry).await,
            Self::Studio(studio_backend) => studio_backend.sync(asset, lockfile_entry).await,
        }
    }
}

#[derive(Debug)]
enum Event {
    Discovered(PathBuf),
    InFlight(PathBuf),
    Finished {
        state: EventState,
        input_name: String,
        path: PathBuf,
        rel_path: RelativePathBuf,
        hash: Hash,
        asset_ref: Option<AssetRef>,
        node: Option<Box<codegen::Node>>,
        sprite_info: Option<SpriteInfo>,
    },
    Failed(PathBuf),
}

#[derive(Debug)]
enum EventState {
    Synced { new: bool },
    Duplicate,
}

pub async fn sync(args: SyncArgs, mp: MultiProgress) -> anyhow::Result<()> {
    let config = Config::read_from(args.project.clone()).await?;
    let target = args.target();

    let existing_lockfile = RawLockfile::read_from(&config.project_dir)
        .await?
        .into_lockfile()?;

    let font_db = Arc::new({
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    });

    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();

    let collector_handle = tokio::spawn({
        let inputs = config.inputs.clone();
        async move { collect_events(event_rx, target, inputs, mp).await }
    });

    let params = walk::Params {
        args: args.clone(),
        target,
        existing_lockfile,
        font_db,
        backend: {
            let params = backend::Params {
                api_key: args.api_key.clone(),
                creator: config.creator.clone(),
                expected_price: args.expected_price,
                project_dir: config.project_dir.clone(),
            };
            match &target {
                SyncTarget::Cloud { dry_run: false } => {
                    Some(TargetBackend::Cloud(backend::Cloud::new(params).await?))
                }
                SyncTarget::Cloud { dry_run: true } => None,
                SyncTarget::Debug => Some(TargetBackend::Debug(backend::Debug::new(params).await?)),
                SyncTarget::Studio => {
                    Some(TargetBackend::Studio(backend::Studio::new(params).await?))
                }
            }
        },
    };

    walk::walk(params, &config, &event_tx).await;
    drop(event_tx);

    let results = collector_handle.await??;

    if matches!(target, SyncTarget::Cloud { dry_run: true }) {
        if results.new_count > 0 {
            bail!("Dry run: {} new assets would be synced", results.new_count)
        } else {
            info!("Dry run: No new assets would be synced");
            return Ok(());
        }
    }

    if target.write_on_sync() {
        results.new_lockfile.write_to(&config.project_dir).await?;
    }

    let mut total_codegen_files = 0;
    let mut total_assets = 0;
    let mut total_web_assets = 0;

    for (input_name, source) in results.input_sources {
        let input = config
            .inputs
            .get(&input_name)
            .context("Failed to find input for codegen input")?;

        let asset_count = source.len();
        total_assets += asset_count;
        total_web_assets += input.web.len();

        let mut langs_to_generate = vec![codegen::Language::Luau];

        if config.codegen.typescript {
            langs_to_generate.push(codegen::Language::TypeScript);
        }

        for lang in langs_to_generate {
            let node = codegen::create_node(&source, &config.codegen);
            let ext = match lang {
                codegen::Language::Luau => "luau",
                codegen::Language::TypeScript => "d.ts",
            };
            let code = codegen::generate_code(
                lang,
                &input_name,
                &node,
                &config.codegen.input_naming_convention,
            )?;

            let output_path = config.project_dir.join(&input.output_path);
            fs::create_dir_all(&output_path).await?;
            let output_file = output_path.join(format!("{input_name}.{ext}"));
            fs::write(&output_file, code).await?;
            total_codegen_files += 1;
            info!(
                "Generated {} with {} asset(s)",
                output_file.display(),
                asset_count
            );
        }
    }

    info!(
        "Sync complete: {} codegen file(s) generated, {} asset reference(s), {} lockfile entries, {} web asset(s) from configuration",
        total_codegen_files,
        total_assets,
        results.new_lockfile.count_entries(),
        total_web_assets
    );

    if results.any_failed {
        bail!("Some assets failed to sync")
    }

    Ok(())
}

fn should_pack(input: &Input, args: &SyncArgs) -> bool {
    if args.pack {
        return true;
    }
    if args.no_pack {
        return false;
    }

    input.pack.as_ref().is_some_and(|pack| pack.enabled)
}

fn apply_pack_overrides(base_options: Option<&PackOptions>, args: &SyncArgs) -> PackOptions {
    let mut options = base_options.cloned().unwrap_or_default();

    if args.pack {
        options.enabled = true;
    }
    if args.no_pack {
        options.enabled = false;
    }
    if let Some(max_size) = args.pack_max_size {
        options.max_size = max_size;
    }
    if let Some(padding) = args.pack_padding {
        options.padding = padding;
    }
    if let Some(extrude) = args.pack_extrude {
        options.extrude = extrude;
    }
    if let Some(algorithm) = args.pack_algorithm.clone() {
        options.algorithm = algorithm;
    }
    if args.pack_trim {
        options.allow_trim = true;
    }
    if args.pack_no_trim {
        options.allow_trim = false;
    }
    if let Some(page_limit) = args.pack_page_limit {
        options.page_limit = Some(page_limit);
    }
    if let Some(sort) = args.pack_sort.clone() {
        options.sort = sort;
    }
    if args.pack_dedupe {
        options.dedupe = true;
    }

    options
}

struct PackingMetadata {
    manifest: pack::manifest::AtlasManifest,
    sprite_to_path: HashMap<String, RelativePathBuf>,
    sprite_to_hash: HashMap<String, Hash>,
}

fn pack_assets(
    assets: Vec<Asset>,
    input_name: &str,
    input: &Input,
    args: &SyncArgs,
) -> anyhow::Result<(Vec<Asset>, Option<PackingMetadata>)> {
    let pack_options = apply_pack_overrides(input.pack.as_ref(), args);
    let packer = Packer::new(pack_options);

    let (packable_assets, mut result_assets): (Vec<_>, Vec<_>) = assets
        .into_iter()
        .partition(|asset| matches!(asset.ty, crate::asset::AssetType::Image(_)));

    if packable_assets.is_empty() {
        return Ok((result_assets, None));
    }

    let mut sprite_to_path = HashMap::new();
    let mut sprite_to_hash = HashMap::new();
    for asset in &packable_assets {
        if let Some(name) = asset.path.file_stem() {
            sprite_to_path.insert(name.to_string(), asset.path.clone());
            sprite_to_hash.insert(name.to_string(), asset.hash);
        }
    }

    let pack_result = packer.pack_assets(&packable_assets, input_name)?;
    if pack_result.atlases.is_empty() {
        return Ok((result_assets, None));
    }

    for atlas in &pack_result.atlases {
        let filename = format!("{}-sheet-{}.png", input_name, atlas.page_index);
        let sync_path = RelativePathBuf::from(filename);
        let atlas_asset = Asset::new(sync_path, atlas.image_data.clone().into())?;
        result_assets.push(atlas_asset);
    }

    Ok((
        result_assets,
        Some(PackingMetadata {
            manifest: pack_result.manifest,
            sprite_to_path,
            sprite_to_hash,
        }),
    ))
}

fn atlas_node(image: String, sprite_info: &pack::manifest::SpriteInfo) -> codegen::Node {
    codegen::Node::AtlasSprite(codegen::AtlasSpriteData {
        image,
        rect: sprite_info.rect,
        size: sprite_info.source_size,
        trimmed: sprite_info.trimmed,
        sprite_source_size: sprite_info.sprite_source_size,
    })
}
