use super::SyncState;
use crate::{
    asset::{Asset, AssetKind, ModelFileFormat, ModelKind},
    config::Input,
    util::{alpha_bleed::alpha_bleed, svg::svg_to_png},
};
use anyhow::{bail, Context};
use image::DynamicImage;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info, warn};
use rbx_xml::DecodeOptions;
use std::{io::Cursor, sync::Arc};

pub async fn process_input(
    state: Arc<SyncState>,
    input: &Input,
    assets: Vec<Asset>,
) -> anyhow::Result<()> {
    let progress_bar = state.multi_progress.add(
        ProgressBar::new(assets.len() as u64)
            .with_prefix(input.name.clone())
            .with_style(
                ProgressStyle::default_bar()
                    .template("Input \"{prefix}\"\n {msg}\n Progress: {pos}/{len} | ETA: {eta}\n[{bar:40.cyan/blue}]")
                    .unwrap()
                    .progress_chars("=> "),
            ),
    );

    for mut asset in assets {
        let display = asset.path.display().to_string();

        let message = format!("Processing \"{}\"", display);
        progress_bar.set_message(message);
        progress_bar.inc(1);

        if state.args.dry_run {
            info!("File {} would be synced", display);
            continue;
        } else {
            debug!("File {} changed, syncing", display);
        }

        if let Err(err) = process_asset(state.clone(), input, &mut asset).await {
            warn!(
                "Skipping file {} because it failed processing: {}",
                display, err
            );
            continue;
        }
    }

    Ok(())
}

async fn process_asset(
    state: Arc<SyncState>,
    input: &Input,
    asset: &mut Asset,
) -> anyhow::Result<()> {
    let ext = asset.path.extension().context("File has no extension")?;
    if ext == "svg" {
        asset.data = svg_to_png(&asset.data, state.font_db.clone()).await?;
    }

    match asset.kind {
        AssetKind::Model(ModelKind::Animation(ref format)) => {
            asset.data = get_animation(&asset.data, format)?;
        }
        AssetKind::Decal(_) if input.bleed => {
            let mut image: DynamicImage = image::load_from_memory(&asset.data)?;
            alpha_bleed(&mut image);

            let mut writer = Cursor::new(Vec::new());
            image.write_to(&mut writer, image::ImageFormat::Png)?;
            asset.data = writer.into_inner();
        }
        _ => {}
    };

    Ok(())
}

fn get_animation(data: &[u8], format: &ModelFileFormat) -> anyhow::Result<Vec<u8>> {
    let dom = match format {
        ModelFileFormat::Binary => rbx_binary::from_reader(data)?,
        ModelFileFormat::Xml => rbx_xml::from_reader(data, DecodeOptions::new())?,
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
