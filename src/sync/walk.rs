use super::{SyncState, WalkedFile};
use crate::{
    asset::{AssetKind, AudioKind, DecalKind, ModelFileFormat, ModelKind},
    config::Input,
};
use anyhow::{bail, Context};
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use walkdir::WalkDir;

pub async fn walk(state: Arc<SyncState>, input: &Input) -> anyhow::Result<Vec<WalkedFile>> {
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

        match walk_file(path.clone()).await {
            Ok(file) => files.push(file),
            Err(err) => {
                warn!("Skipping file {}: {}", path.display(), err);
            }
        }
    }

    progress_bar.set_message("Done reading files");

    Ok(files)
}

async fn walk_file(path: PathBuf) -> anyhow::Result<WalkedFile> {
    let data = fs::read(&path).await?;
    let ext = path.extension().context("File has no extension")?;
    let ext = ext.to_str().context("Extension is not valid UTF-8")?;

    let kind = kind_from_ext(ext)?;

    Ok(WalkedFile { path, data, kind })
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
