use std::io::Cursor;

use anyhow::{bail, Context};
use rbx_xml::DecodeOptions;

use crate::asset::ModelFileFormat;

pub fn get_animation(data: &[u8], format: &ModelFileFormat) -> anyhow::Result<Vec<u8>> {
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
