use anyhow::Context;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

fn asset_path(file_path: &str, strip_dir: &str, strip_extension: bool) -> anyhow::Result<String> {
    if strip_extension {
        Path::new(file_path).with_extension("")
    } else {
        PathBuf::from(file_path)
    }
    .to_str()
    .unwrap()
    .strip_prefix(strip_dir)
    .context("Failed to strip directory prefix")
    .map(|s| s.to_string())
}

pub fn generate_luau(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    let table = assets
        .iter()
        .map(|(file_path, asset_id)| {
            let file_stem = asset_path(file_path, strip_dir, strip_extension)?;
            Ok(format!("\t[\"{}\"] = \"{}\"", file_stem, asset_id))
        })
        .collect::<Result<Vec<String>, anyhow::Error>>()?
        .join(",\n");

    Ok(format!("return {{\n{}\n}}", table))
}

pub fn generate_ts(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    output_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    let interface = assets
        .keys()
        .map(|file_path| {
            let file_stem = asset_path(file_path, strip_dir, strip_extension)?;
            Ok(format!("\t\"{}\": string", file_stem))
        })
        .collect::<Result<Vec<String>, anyhow::Error>>()?
        .join(",\n");

    Ok(format!(
        "declare const {}: {{\n{}\n}}\nexport = {}",
        output_dir, interface, output_dir
    ))
}
