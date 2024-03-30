use crate::lockfile::LockFile;
use anyhow::Context;
use std::path::Path;

pub fn generate_lua(lockfile: &LockFile, strip_dir: &str) -> anyhow::Result<String> {
    let table = lockfile
        .entries
        .iter()
        .map(|(file_path, file_entry)| {
            let file_stem = Path::new(file_path)
                .to_str()
                .context("Failed to convert path to string")?
                .strip_prefix(strip_dir)
                .context("Failed to strip directory prefix")?;
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
) -> anyhow::Result<String> {
    let interface = lockfile
        .entries
        .keys()
        .map(|file_path| {
            let file_stem = Path::new(file_path)
                .to_str()
                .context("Failed to convert path to string")?
                .strip_prefix(strip_dir)
                .context("Failed to strip directory prefix")?;
            Ok(format!("\t\"{}\": string", file_stem))
        })
        .collect::<Result<Vec<String>, anyhow::Error>>()?
        .join(",\n");

    Ok(format!(
        "declare const {}: {{\n{}\n}}\nexport = {}",
        output_dir, interface, output_dir
    ))
}
