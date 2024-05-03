use crate::lockfile::LockFile;
use anyhow::Context;
use std::path::Path;

fn asset_path(file_path: &str, strip_dir: &str, strip_extension: bool) -> anyhow::Result<String> {
    let file_path = Path::new(file_path);
    if strip_extension {
        file_path.with_extension("")
    } else {
        file_path.to_owned()
    }
    .to_str()
    .context("Failed to convert path to string")?
    .strip_prefix(strip_dir)
    .context("Failed to strip directory prefix")
    .map(|s| s.to_string())
}

pub fn generate_lua(
    lockfile: &LockFile,
    strip_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    let table = lockfile
        .entries
        .iter()
        .map(|(file_path, file_entry)| {
            let file_stem = asset_path(&file_path, &strip_dir, strip_extension)?;
            Ok(format!(
                "\t[\"{}\"] = \"rbxassetid://{}\"",
                file_stem, file_entry.asset_id
            ))
        })
        .collect::<Result<Vec<String>, anyhow::Error>>()?
        .join(",\n");

    Ok(format!("return {{\n{}\n}}", table))
}

pub fn generate_ts(
    lockfile: &LockFile,
    strip_dir: &str,
    output_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    let interface = lockfile
        .entries
        .keys()
        .map(|file_path| {
            let file_stem = asset_path(&file_path, &strip_dir, strip_extension)?;
            Ok(format!("\t\"{}\": string", file_stem))
        })
        .collect::<Result<Vec<String>, anyhow::Error>>()?
        .join(",\n");

    Ok(format!(
        "declare const {}: {{\n{}\n}}\nexport = {}",
        output_dir, interface, output_dir
    ))
}
