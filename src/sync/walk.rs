use super::SyncState;
use crate::{
    asset::{Asset, AssetKind, AudioKind, DecalKind, ModelFileFormat, ModelKind},
    config::Input,
};
use anyhow::{bail, Context};
use blake3::Hasher;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, warn};
use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use walkdir::WalkDir;

pub async fn walk(state: Arc<SyncState>, input: &Input) -> anyhow::Result<Vec<Asset>> {
    let prefix = input.path.get_prefix();

    let prefix_display = prefix.to_string_lossy().to_string();
    let progress_bar = state
        .multi_progress
        .add(
            ProgressBar::new_spinner()
                .with_prefix(prefix_display)
                .with_style(
                    ProgressStyle::default_spinner()
                        .template("{prefix:.bold}: {spinner} {msg}")
                        .unwrap(),
                ),
        )
        .with_message("Collecting files");

    let entries = WalkDir::new(prefix)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| input.path.is_match(entry.path()) && entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();

    let mut files = Vec::new();
    for path in entries {
        progress_bar.set_message(format!("Reading {}", path.display()));
        progress_bar.tick();

        match walk_file(state.clone(), path.clone()).await {
            Ok(WalkFileResult {
                asset,
                changed: true,
            }) => files.push(asset),
            Ok(WalkFileResult {
                changed: false,
                asset: _,
            }) => {
                debug!("Skipping file {} because it didn't change", path.display());
            }
            Err(err) => {
                warn!("Skipping file {}: {}", path.display(), err);
            }
        }
    }

    progress_bar.set_message("Done reading files");

    Ok(files)
}

struct WalkFileResult {
    asset: Asset,
    changed: bool,
}

async fn walk_file(state: Arc<SyncState>, path: PathBuf) -> anyhow::Result<WalkFileResult> {
    let data = fs::read(&path).await?;
    let ext = path.extension().context("File has no extension")?;
    let ext = ext.to_str().context("Extension is not valid UTF-8")?;

    let kind = kind_from_ext(ext)?;

    let hash = hash_file(&data);
    let entry = state
        .existing_lockfile
        .entries
        .get(&path.to_string_lossy().to_string());

    let changed = entry.is_none_or(|entry| entry.hash != hash);

    let asset = Asset { path, data, kind };

    Ok(WalkFileResult { asset, changed })
}

fn hash_file(data: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize().to_string()
}

fn kind_from_ext(ext: &str) -> anyhow::Result<AssetKind> {
    let kind = match ext {
        "mp3" => AssetKind::Audio(AudioKind::Mp3),
        "ogg" => AssetKind::Audio(AudioKind::Ogg),
        "flac" => AssetKind::Audio(AudioKind::Flac),
        "wav" => AssetKind::Audio(AudioKind::Wav),
        "png" | "svg" => AssetKind::Decal(DecalKind::Png),
        "jpg" => AssetKind::Decal(DecalKind::Jpg),
        "bmp" => AssetKind::Decal(DecalKind::Bmp),
        "tga" => AssetKind::Decal(DecalKind::Tga),
        "fbx" => AssetKind::Model(ModelKind::Model),
        "rbxm" | "rbxmx" => {
            let format = if ext == "rbxm" {
                ModelFileFormat::Binary
            } else {
                ModelFileFormat::Xml
            };

            AssetKind::Model(ModelKind::Animation(format))
        }
        _ => bail!("Unknown extension .{ext}"),
    };

    Ok(kind)
}
