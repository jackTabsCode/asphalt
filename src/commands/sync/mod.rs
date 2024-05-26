use self::state::SyncState;
use crate::{cli::SyncArgs, FileEntry};
use anyhow::{anyhow, Context};
use blake3::Hasher;
use codegen::{generate_lua, generate_ts};
use config::SyncConfig;
use console::style;
use image::{DynamicImage, ImageFormat};
use rbxcloud::rbx::v1::assets::AssetType;
use std::{collections::VecDeque, io::Cursor, path::Path};
use tokio::fs::{read, read_dir, read_to_string, write, DirEntry};
use upload::upload_asset;
use util::{alpha_bleed::alpha_bleed, svg::svg_to_png};

mod codegen;
pub mod config;
mod state;
mod upload;
mod util;

fn fix_path(path: &str) -> String {
    path.replace('\\', "/")
}

async fn check_file(entry: &DirEntry, state: &SyncState) -> anyhow::Result<Option<FileEntry>> {
    let path = entry.path();
    let path_str = path.to_str().context("Failed to convert path to string")?;
    let fixed_path = fix_path(path_str);

    let mut bytes = read(&path)
        .await
        .with_context(|| format!("Failed to read {}", fixed_path))?;

    let mut extension = match path.extension().and_then(|s| s.to_str()) {
        Some(extension) => extension,
        None => return Ok(None),
    };

    if extension == "svg" {
        bytes = svg_to_png(&bytes, &state.font_db)
            .await
            .with_context(|| format!("Failed to convert SVG to PNG: {}", fixed_path))?;
        extension = "png";
    }

    let asset_type = match AssetType::try_from_extension(extension) {
        Ok(asset_type) => asset_type,
        Err(e) => {
            eprintln!(
                "Skipping {} because it has an unsupported extension: {}",
                style(fixed_path).yellow(),
                e
            );
            return Ok(None);
        }
    };

    #[cfg(feature = "alpha_bleed")]
    match asset_type {
        AssetType::DecalJpeg | AssetType::DecalBmp | AssetType::DecalPng => {
            let mut image: DynamicImage = image::load_from_memory(&bytes)?;
            alpha_bleed(&mut image);

            let format = ImageFormat::from_extension(extension).ok_or(anyhow!(
                "Failed to get image format from extension: {}",
                extension
            ))?;

            let mut new_bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
            image.write_to(&mut new_bytes, format)?;

            bytes = new_bytes.into_inner();
        }
        _ => {}
    }

    let mut hasher = Hasher::new();
    hasher.update(&bytes);
    let hash = hasher.finalize().to_string();

    let existing = state.existing_lockfile.entries.get(fixed_path.as_str());

    if let Some(existing_value) = existing {
        if existing_value.hash == hash {
            return Ok(Some(FileEntry {
                hash,
                asset_id: existing_value.asset_id,
            }));
        }
    }

    let file_name = path
        .file_name()
        .with_context(|| format!("Failed to get file name of {}", fixed_path))?
        .to_str()
        .with_context(|| format!("Failed to convert file name to string: {}", fixed_path))?;

    let asset_id = upload_asset(
        bytes,
        file_name,
        asset_type,
        state.api_key.clone(),
        state.creator.clone(),
    )
    .await
    .with_context(|| format!("Failed to upload {}", fixed_path))?;

    eprintln!("Uploaded {}", style(fixed_path).green());

    Ok(Some(FileEntry { hash, asset_id }))
}

pub async fn sync(args: SyncArgs) -> anyhow::Result<()> {
    let config: SyncConfig = {
        let file_contents = read_to_string("asphalt.toml")
            .await
            .context("Failed to read asphalt.toml")?;
        toml::from_str(&file_contents).context("Failed to parse config")
    }?;

    let mut state = SyncState::new(args, config)
        .await
        .context("Failed to create state")?;

    eprintln!("{}", style("Syncing...").dim());

    let mut remaining_items = VecDeque::new();
    remaining_items.push_back(state.asset_dir.clone());

    while let Some(path) = remaining_items.pop_front() {
        let mut dir_entries = read_dir(path.clone()).await.with_context(|| {
            format!(
                "Failed to read directory: {}",
                path.to_str().unwrap_or("???")
            )
        })?;

        while let Some(entry) = dir_entries
            .next_entry()
            .await
            .with_context(|| format!("Failed to read directory entry: {:?}", path))?
        {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                remaining_items.push_back(entry_path);
            } else {
                let result = match check_file(&entry, &state).await {
                    Ok(Some(result)) => result,
                    Ok(None) => continue,
                    Err(e) => {
                        eprintln!("{} {:?}", style("Error:").red(), e);
                        continue;
                    }
                };

                let path_str = entry_path.to_str().with_context(|| {
                    format!("Failed to convert path to string: {:?}", entry_path)
                })?;
                let fixed_path = fix_path(path_str);

                state.new_lockfile.entries.insert(fixed_path, result);
            }
        }
    }

    write(
        "asphalt.lock.toml",
        toml::to_string(&state.new_lockfile).context("Failed to serialize lockfile")?,
    )
    .await
    .context("Failed to write lockfile")?;

    let asset_dir_str = state
        .asset_dir
        .to_str()
        .context("Failed to convert asset directory to string")?;

    state
        .new_lockfile
        .entries
        .extend(state.existing.into_iter().map(|(path, asset)| {
            (
                path,
                FileEntry {
                    hash: "".to_string(),
                    asset_id: asset.id,
                },
            )
        }));

    let lua_filename = format!("{}.{}", state.output_name, state.lua_extension);
    let lua_output = generate_lua(
        &state.new_lockfile,
        asset_dir_str,
        &state.style,
        state.strip_extension,
    );

    write(Path::new(&state.write_dir).join(lua_filename), lua_output?)
        .await
        .context("Failed to write output Lua file")?;

    if state.typescript {
        let ts_filename = format!("{}.d.ts", state.output_name);
        let ts_output = generate_ts(
            &state.new_lockfile,
            asset_dir_str,
            state.output_name.as_str(),
            &state.style,
            state.strip_extension,
        );

        write(Path::new(&state.write_dir).join(ts_filename), ts_output?)
            .await
            .context("Failed to write output TypeScript file")?;
    }

    eprintln!("{}", style("Synced!").dim());

    Ok(())
}
