use std::collections::BTreeMap;
use std::fmt::Write;

use ast::{AstTarget, Expression, ReturnStatement};

use crate::commands::sync::config::CodegenStyle;

mod ast;
mod flat;
mod nested;

pub fn generate_luau(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    style: &CodegenStyle,
    strip_extension: bool,
) -> anyhow::Result<String> {
    match style {
        CodegenStyle::Flat => flat::generate_luau(assets, strip_dir, strip_extension),
        CodegenStyle::Nested => nested::generate_luau(assets, strip_dir, strip_extension),
    }
}

pub fn generate_ts(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    output_dir: &str,
    style: &CodegenStyle,
    strip_extension: bool,
) -> anyhow::Result<String> {
    match style {
        CodegenStyle::Flat => flat::generate_ts(assets, strip_dir, output_dir, strip_extension),
        CodegenStyle::Nested => nested::generate_ts(assets, strip_dir, output_dir, strip_extension),
    }
}

fn generate_code(expression: Expression, target: AstTarget) -> anyhow::Result<String> {
    let mut buffer = String::new();
    write!(buffer, "{}", ReturnStatement(expression, target))?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    fn test_assets() -> BTreeMap<String, String> {
        let mut entries = BTreeMap::new();
        entries.insert("assets/foo.png".to_string(), "rbxassetid://1".to_string());
        entries.insert(
            "assets/bar/baz.png".to_string(),
            "rbxasset://.asphalt/bar/baz.png".to_string(),
        );
        entries
    }

    #[test]
    fn generate_luau() {
        let lockfile = test_assets();

        let lua = super::flat::generate_luau(&lockfile, "assets", false).unwrap();
        assert_eq!(lua, "return {\n\t[\"/bar/baz.png\"] = \"rbxasset://.asphalt/bar/baz.png\",\n\t[\"/foo.png\"] = \"rbxassetid://1\",\n}\n");

        let lua = super::flat::generate_luau(&lockfile, "assets", true).unwrap();
        assert_eq!(
            lua,
            "return {\n\t[\"/bar/baz\"] = \"rbxasset://.asphalt/bar/baz.png\",\n\t[\"/foo\"] = \"rbxassetid://1\",\n}\n"
        );
    }

    #[test]
    fn generate_ts() {
        let lockfile = test_assets();

        let ts = super::flat::generate_ts(&lockfile, "assets", "assets", false).unwrap();
        assert_eq!(ts, "declare const assets: {\n\t\"/bar/baz.png\": \"rbxasset://.asphalt/bar/baz.png\";\n\t\"/foo.png\": \"rbxassetid://1\";\n};\nexport = assets;\n");

        let ts = super::flat::generate_ts(&lockfile, "assets", "assets", true).unwrap();
        assert_eq!(ts, "declare const assets: {\n\t\"/bar/baz\": \"rbxasset://.asphalt/bar/baz.png\";\n\t\"/foo\": \"rbxassetid://1\";\n};\nexport = assets;\n");
    }

    #[test]
    fn generate_luau_nested() {
        let lockfile = test_assets();

        let lua = super::nested::generate_luau(&lockfile, "assets", false).unwrap();
        assert_eq!(
            lua,
            "return {\n\tbar = {\n\t\t[\"baz.png\"] = \"rbxasset://.asphalt/bar/baz.png\",\n\t},\n\t[\"foo.png\"] = \"rbxassetid://1\",\n}\n"
        );

        let lua = super::nested::generate_luau(&lockfile, "assets", true).unwrap();
        assert_eq!(
            lua,
            "return {\n\tbar = {\n\t\tbaz = \"rbxasset://.asphalt/bar/baz.png\",\n\t},\n\tfoo = \"rbxassetid://1\",\n}\n"
        );
    }

    #[test]
    fn generate_ts_nested() {
        let lockfile = test_assets();

        let ts = super::nested::generate_ts(&lockfile, "assets", "assets", false).unwrap();
        assert_eq!(
            ts,
            "declare const assets: {\n\tbar: {\n\t\t\"baz.png\": \"rbxasset://.asphalt/bar/baz.png\";\n\t};\n\t\"foo.png\": \"rbxassetid://1\";\n};\nexport = assets;\n"
        );

        let ts = super::nested::generate_ts(&lockfile, "assets", "assets", true).unwrap();
        assert_eq!(
            ts,
            "declare const assets: {\n\tbar: {\n\t\tbaz: \"rbxasset://.asphalt/bar/baz.png\";\n\t};\n\tfoo: \"rbxassetid://1\";\n};\nexport = assets;\n"
        );
    }
}
