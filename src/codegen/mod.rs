use crate::{config::StyleType, LockFile};

mod flat;
mod nested;

pub fn generate_lua(
    lockfile: &LockFile,
    strip_dir: &str,
    style: &StyleType,
) -> anyhow::Result<String> {
    match style {
        StyleType::Flat => flat::generate_lua(lockfile, strip_dir),
        StyleType::Nested => nested::generate_lua(lockfile, strip_dir),
    }
}

pub fn generate_ts(
    lockfile: &LockFile,
    strip_dir: &str,
    output_dir: &str,
    style: &StyleType,
) -> anyhow::Result<String> {
    match style {
        StyleType::Flat => flat::generate_ts(lockfile, strip_dir, output_dir),
        StyleType::Nested => nested::generate_ts(lockfile, strip_dir, output_dir),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{FileEntry, LockFile};

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

        let lua = super::flat::generate_lua(&lockfile, "assets").unwrap();
        assert_eq!(lua, "return {\n\t[\"/bar/baz.png\"] = \"rbxassetid://2\",\n\t[\"/foo.png\"] = \"rbxassetid://1\"\n}");
    }

    #[test]
    fn generate_ts() {
        let lockfile = test_lockfile();

        let ts = super::flat::generate_ts(&lockfile, "assets", "assets").unwrap();
        assert_eq!(ts, "declare const assets: {\n\t\"/bar/baz.png\": string,\n\t\"/foo.png\": string\n}\nexport = assets");
    }

    #[test]
    fn generate_lua_nested() {
        let lockfile = test_lockfile();

        let lua = super::nested::generate_lua(&lockfile, "assets").unwrap();
        assert_eq!(
            lua,
            "return {\n    bar = {\n        [\"baz.png\"] = \"rbxassetid://2\",\n    },\n    [\"foo.png\"] = \"rbxassetid://1\",\n}");
    }

    #[test]
    fn generate_ts_nested() {
        let lockfile = test_lockfile();

        let ts = super::nested::generate_ts(&lockfile, "assets", "assets").unwrap();
        assert_eq!(
            ts,
            "declare const assets: {\n    bar: {\n        \"baz.png\": \"rbxassetid://2\",\n    },\n    \"foo.png\": \"rbxassetid://1\",\n}\nexport = assets");
    }
}
