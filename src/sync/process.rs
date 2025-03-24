use super::{ProcessedFile, SyncState};
use crate::{
    asset::{AssetKind, ModelFileFormat, ModelKind},
    config::Input,
    sync::WalkedFile,
    util::{alpha_bleed::alpha_bleed, svg::svg_to_png},
};
use anyhow::{bail, Context};
use blake3::Hasher;
use image::DynamicImage;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, warn};
use rbx_xml::DecodeOptions;
use std::{io::Cursor, sync::Arc};

pub async fn process_input(
    state: Arc<SyncState>,
    input: &Input,
    files: Vec<WalkedFile>,
) -> anyhow::Result<()> {
    let progress_bar = state.multi_progress.add(
        ProgressBar::new(files.len() as u64)
            .with_prefix(input.name.clone())
            .with_style(
                ProgressStyle::default_bar()
                    .template("Input \"{prefix}\"\n {msg}\n Progress: {pos}/{len} | ETA: {eta}\n[{bar:40.cyan/blue}]")
                    .unwrap()
                    .progress_chars("=> "),
            ),
    );

    for file in files {
        let file_path_display = file.path.to_string_lossy().to_string();
        let message = format!("Processing \"{}\"", file_path_display);
        progress_bar.set_message(message);
        progress_bar.inc(1);

        match process_file(state.clone(), input, file).await {
            Ok(processed) => {
                if processed.changed {
                    debug!("File {} changed, uploading", processed.file.path.display());
                    // thread::sleep(std::time::Duration::from_secs(1));
                } else {
                    debug!("File {} unchanged, skipping", processed.file.path.display());
                }
            }
            Err(err) => {
                warn!("Failed to process file {}: {}", file_path_display, err);
            }
        }
    }

    Ok(())
}

async fn process_file(
    state: Arc<SyncState>,
    input: &Input,
    mut file: WalkedFile,
) -> anyhow::Result<ProcessedFile> {
    let hash = hash_file(&file.data);
    let entry = state
        .lockfile
        .entries
        .get(&file.path.to_string_lossy().to_string());

    let changed = entry.is_none_or(|entry| entry.hash != hash);

    let ext = file.path.extension().context("File has no extension")?;
    if ext == "svg" {
        file.data = svg_to_png(&file.data, state.font_db.clone()).await?;
    }

    file.data = match file.kind {
        AssetKind::Model(ModelKind::Animation(_)) => {
            get_animation(file.data, ModelFileFormat::Binary)?
        }
        AssetKind::Decal(_) if input.bleed => {
            let mut image: DynamicImage = image::load_from_memory(&file.data)?;
            alpha_bleed(&mut image);

            let mut writer = Cursor::new(Vec::new());
            image.write_to(&mut writer, image::ImageFormat::Png)?;
            writer.into_inner()
        }
        _ => file.data,
    };

    Ok(ProcessedFile { file, changed })
}

fn hash_file(data: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize().to_string()
}

fn get_animation(data: Vec<u8>, format: ModelFileFormat) -> anyhow::Result<Vec<u8>> {
    let slice = data.as_slice();
    let dom = match format {
        ModelFileFormat::Binary => rbx_binary::from_reader(slice)?,
        ModelFileFormat::Xml => rbx_xml::from_reader(slice, DecodeOptions::new())?,
    };

    let children = dom.root().children();

    let first_ref = *children.first().context("No children found in root")?;
    let first = dom
        .get_by_ref(first_ref)
        .context("Failed to get first child")?;

    if first.class != "KeyframeSequence" {
        bail!(
            r"Root class name of this model is not KeyframeSequence. Asphalt expects Roblox model files (.rbxm/.rbxmx) to be animations (regular models can't be uploaded with Open Cloud). If you did not expect this error, don't include this file in this input."
        )
    }

    let mut writer = Cursor::new(Vec::new());
    rbx_binary::to_writer(&mut writer, &dom, &[first_ref])?;

    Ok(writer.into_inner())
}
