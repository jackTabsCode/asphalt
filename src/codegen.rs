use crate::lockfile::LockFile;
use std::path::Path;

pub fn generate_lua(lockfile: &LockFile, strip_dir: &str) -> String {
    let table = lockfile
        .entries
        .iter()
        .map(|(file_path, file_entry)| {
            let file_stem = Path::new(file_path)
                .to_str()
                .unwrap()
                .strip_prefix(strip_dir)
                .unwrap();
            format!(
                "\t[\"{}\"] = \"rbxassetid://{}\"",
                file_stem, file_entry.asset_id
            )
        })
        .collect::<Vec<String>>()
        .join(",\n");

    format!("return {{\n{}\n}}", table)
}

pub fn generate_ts(lockfile: &LockFile, strip_dir: &str, output_dir: &str) -> String {
    let interface = lockfile
        .entries
        .keys()
        .map(|file_path| {
            let file_stem = Path::new(file_path)
                .to_str()
                .unwrap()
                .strip_prefix(strip_dir)
                .unwrap();
            format!("\t\"{}\": string", file_stem)
        })
        .collect::<Vec<String>>()
        .join(",\n");

    format!(
        "declare const {}: {{\n{}\n}}\nexport = {}",
        output_dir, interface, output_dir
    )
}
