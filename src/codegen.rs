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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{tarmac, FileEntry, LockFile};

    fn test_lockfile() -> LockFile {
        let mut entries = BTreeMap::new();
        entries.insert(
            "assets/foo.png".to_string(),
            FileEntry {
                asset_id: 1,
                hash: "a".to_string(),
            },
        );
        entries.insert(
            "assets/bar/baz.png".to_string(),
            FileEntry {
                asset_id: 2,
                hash: "b".to_string(),
            },
        );
        LockFile { entries }
    }

    #[test]
    fn generate_lua() {
        let lockfile = test_lockfile();

        let lua = super::generate_lua(&lockfile, "assets").unwrap();
        assert_eq!(lua, "return {\n\t[\"/bar/baz.png\"] = \"rbxassetid://2\",\n\t[\"/foo.png\"] = \"rbxassetid://1\"\n}");
    }

    #[test]
    fn generate_ts() {
        let lockfile = test_lockfile();

        let ts = super::generate_ts(&lockfile, "assets", "assets").unwrap();
        assert_eq!(ts, "declare const assets: {\n\t\"/bar/baz.png\": string,\n\t\"/foo.png\": string\n}\nexport = assets");
    }

    #[test]
    fn generate_lua_tarmac() {
        let lockfile = test_lockfile();

        let lua = tarmac::generate_lua(&lockfile, "assets").unwrap();
        assert_eq!(
            lua,
            "return {\n    bar = {\n        [\"baz.png\"] = \"rbxassetid://2\",\n    },\n    [\"foo.png\"] = \"rbxassetid://1\",\n}");
    }

    #[test]
    fn generate_ts_tarmac() {
        let lockfile = test_lockfile();

        let ts = tarmac::generate_ts(&lockfile, "assets", "assets").unwrap();
        assert_eq!(
            ts,
            "declare const assets: {\n    bar: {\n        \"baz.png\": \"rbxassetid://2\",\n    },\n    \"foo.png\": \"rbxassetid://1\",\n}\nexport = assets");
    }
}
