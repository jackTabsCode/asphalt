use crate::glob::Glob;
use anyhow::Context;
use clap::ValueEnum;
use fs_err::tokio as fs;
use relative_path::RelativePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[schemars(description = "Asphalt configuration file")]
pub struct Config {
    #[schemars(description = "Roblox creator information (user or group)")]
    pub creator: Creator,

    #[serde(default)]
    #[schemars(description = "Code generation settings for asset references")]
    pub codegen: Codegen,

    #[schemars(description = "Asset input configurations mapped by name")]
    pub inputs: HashMap<String, Input>,

    #[serde(skip)]
    pub project_dir: PathBuf,
}

pub type InputMap = HashMap<String, Input>;

pub const CONFIG_FILES: &[&str] = &[
    "asphalt.jsonc",
    "asphalt.json",
    "asphalt.json5",
    "asphalt.toml",
];

impl Config {
    pub async fn read_from(project_dir: PathBuf) -> anyhow::Result<Config> {
        for file_name in CONFIG_FILES {
            let config_path = project_dir.join(file_name);
            if fs::metadata(&config_path).await.is_err() {
                continue;
            }

            let content = fs::read_to_string(&config_path).await.with_context(|| {
                format!("Failed to read config file: {}", config_path.display())
            })?;

            let mut config: Config = match *file_name {
                name if name.ends_with(".json") => {
                    let clean_json = fjson::to_json(&content).with_context(|| {
                        format!(
                            "Failed to parse JSON config file: {}",
                            config_path.display()
                        )
                    })?;
                    serde_json::from_str(&clean_json).with_context(|| {
                        format!(
                            "Failed to deserialize JSON config: {}",
                            config_path.display()
                        )
                    })?
                }
                name if name.ends_with(".json5") => {
                    json5::from_str(&content).with_context(|| {
                        format!(
                            "Failed to parse JSON5 config file: {}",
                            config_path.display()
                        )
                    })?
                }
                name if name.ends_with(".jsonc") => {
                    let clean_json = fjson::to_json(&content).with_context(|| {
                        format!(
                            "Failed to parse JSONC config file: {}",
                            config_path.display()
                        )
                    })?;
                    serde_json::from_str(&clean_json).with_context(|| {
                        format!(
                            "Failed to deserialize JSONC config: {}",
                            config_path.display()
                        )
                    })?
                }
                name if name.ends_with(".toml") => toml::from_str(&content).with_context(|| {
                    format!(
                        "Failed to parse TOML config file: {}",
                        config_path.display()
                    )
                })?,
                _ => anyhow::bail!("Unsupported config file format: {file_name}"),
            };

            config.project_dir = project_dir;
            log::info!("Loaded configuration from {}", config_path.display());
            return Ok(config);
        }

        anyhow::bail!(
            "No configuration file found. Please create one of: {}",
            CONFIG_FILES.join(", ")
        )
    }
}

fn default_input_naming_convention() -> InputNamingConvention {
    InputNamingConvention::CamelCase
}

fn default_asset_naming_convention() -> AssetNamingConvention {
    AssetNamingConvention::Preserve
}

#[derive(Debug, Deserialize, Clone, Default, JsonSchema)]
#[serde(default)]
#[schemars(description = "Code generation settings")]
pub struct Codegen {
    #[schemars(
        description = "Code generation style: flat (file path-like) or nested (object property access)"
    )]
    pub style: CodegenStyle,
    #[schemars(description = "Generate TypeScript definition files (.d.ts) in addition to Luau")]
    pub typescript: bool,
    #[schemars(description = "Remove file extensions from generated asset paths")]
    pub strip_extensions: bool,
    #[schemars(description = "Generate Content objects instead of string asset IDs")]
    pub content: bool,
    #[serde(default = "default_input_naming_convention")]
    #[schemars(description = "Naming convention for input module names (default: camel_case)")]
    pub input_naming_convention: InputNamingConvention,
    #[serde(default = "default_asset_naming_convention")]
    #[schemars(
        description = "Naming convention for asset keys in generated code (default: preserve)"
    )]
    pub asset_naming_convention: AssetNamingConvention,
}

#[derive(Debug, Deserialize, Clone, ValueEnum, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(description = "Type of Roblox creator")]
pub enum CreatorType {
    User,
    Group,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[schemars(description = "Roblox creator information")]
pub struct Creator {
    #[serde(rename = "type")]
    #[schemars(description = "Creator type: user or group")]
    pub ty: CreatorType,
    #[schemars(description = "Creator ID (user ID or group ID)")]
    pub id: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[schemars(description = "Input asset configuration")]
pub struct Input {
    #[schemars(with = "String")]
    #[serde(rename = "path")]
    #[schemars(description = "Glob pattern to match asset files (e.g., 'assets/**/*.png')")]
    pub include: Glob,
    #[schemars(description = "Directory where generated code and packed assets will be written")]
    pub output_path: PathBuf,
    #[schemars(description = "Sprite packing/atlas generation configuration (optional)")]
    pub pack: Option<PackOptions>,

    #[serde(default = "default_true")]
    #[schemars(
        description = "Apply alpha bleeding to images to prevent edge artifacts (default: true)"
    )]
    pub bleed: bool,

    #[serde(default)]
    #[schemars(with = "HashMap<PathBuf, WebAsset>")]
    pub web: HashMap<RelativePathBuf, WebAsset>,

    #[serde(default = "default_true")]
    #[schemars(description = "Warn for each duplicate file found (default: true)")]
    pub warn_each_duplicate: bool,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[schemars(description = "Web asset that has already been uploaded to Roblox")]
pub struct WebAsset {
    #[schemars(description = "Roblox asset ID of the uploaded asset")]
    pub id: u64,
}

fn default_pack_max_size() -> (u32, u32) {
    (2048, 2048)
}

fn default_pack_power_of_two() -> bool {
    true
}

fn default_pack_padding() -> u32 {
    2
}

fn default_pack_extrude() -> u32 {
    1
}

fn default_pack_algorithm() -> PackAlgorithm {
    PackAlgorithm::MaxRects
}

fn default_pack_sort() -> PackSort {
    PackSort::Area
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[serde(default)]
#[schemars(description = "Sprite packing configuration")]
pub struct PackOptions {
    #[schemars(description = "Enable sprite packing/atlas generation for this input")]
    pub enabled: bool,
    #[serde(default = "default_pack_max_size")]
    #[schemars(
        description = "Maximum atlas size as (width, height) in pixels (default: 2048x2048)"
    )]
    pub max_size: (u32, u32),
    #[serde(default = "default_pack_power_of_two")]
    #[schemars(description = "Constrain atlas dimensions to power-of-two sizes (default: true)")]
    pub power_of_two: bool,
    #[serde(default = "default_pack_padding")]
    #[schemars(description = "Padding between sprites in pixels (default: 2)")]
    pub padding: u32,
    #[serde(default = "default_pack_extrude")]
    #[schemars(
        description = "Pixels to extrude sprite edges for filtering artifacts (default: 1)"
    )]
    pub extrude: u32,
    #[schemars(description = "Allow trimming transparent borders from sprites (default: false)")]
    pub allow_trim: bool,
    #[serde(default = "default_pack_algorithm")]
    #[schemars(description = "Packing algorithm to use (default: max_rects)")]
    pub algorithm: PackAlgorithm,
    #[schemars(description = "Maximum number of atlas pages to generate")]
    pub page_limit: Option<u32>,
    #[serde(default = "default_pack_sort")]
    #[schemars(description = "Sprite sorting method for deterministic packing (default: area)")]
    pub sort: PackSort,
    #[schemars(description = "Enable deduplication of identical sprites (default: false)")]
    pub dedupe: bool,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            max_size: default_pack_max_size(),
            power_of_two: default_pack_power_of_two(),
            padding: default_pack_padding(),
            extrude: default_pack_extrude(),
            allow_trim: false,
            algorithm: default_pack_algorithm(),
            page_limit: None,
            sort: default_pack_sort(),
            dedupe: false,
        }
    }
}

#[derive(Debug, Deserialize, Clone, ValueEnum, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(description = "Packing algorithm to use")]
pub enum PackAlgorithm {
    MaxRects,
}

#[derive(Debug, Deserialize, Clone, ValueEnum, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(description = "Sprite sorting method for deterministic packing")]
pub enum PackSort {
    Area,
    MaxSide,
    Name,
}

#[derive(Debug, Deserialize, Default, Clone, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(description = "Code generation style")]
pub enum CodegenStyle {
    #[default]
    Flat,
    Nested,
}

#[derive(Debug, Deserialize, Clone, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(description = "Naming convention for input module names")]
#[allow(clippy::enum_variant_names)]
pub enum InputNamingConvention {
    SnakeCase,
    #[default]
    CamelCase,
    PascalCase,
    ScreamingSnakeCase,
}

#[derive(Debug, Deserialize, Clone, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(description = "Naming convention for asset keys in generated code")]
#[allow(clippy::enum_variant_names)]
pub enum AssetNamingConvention {
    SnakeCase,
    CamelCase,
    PascalCase,
    ScreamingSnakeCase,
    KebabCase,
    #[default]
    Preserve,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn minimal_config_toml() -> &'static str {
        r#"[creator]
type = "user"
id = 12345

[inputs.icons]
path = "icons/**/*.png"
output_path = "out/icons.luau"
"#
    }

    fn minimal_config_json() -> &'static str {
        r#"{"creator":{"type":"user","id":12345},"inputs":{"icons":{"path":"icons/**/*.png","output_path":"out/icons.luau"}}}"#
    }

    fn minimal_config_json5() -> &'static str {
        r#"{"creator":{"type":"user","id":12345},"inputs":{"icons":{"path":"icons/**/*.png","output_path":"out/icons.luau"}}}"#
    }

    fn minimal_config_jsonc() -> &'static str {
        r#"{
  "creator": {"type": "user", "id": 12345},
  "inputs": {
    "icons": {"path": "icons/**/*.png", "output_path": "out/icons.luau"}
  }
}"#
    }

    fn setup_temp_config(dir: &PathBuf, file_name: &str, content: &str) {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(file_name);
        fs::write(&path, content).unwrap();
    }

    fn assert_valid_config(config: &Config) {
        assert!(
            matches!(config.creator.ty, CreatorType::User),
            "Creator type should be User, got {:?}",
            config.creator.ty
        );
        assert_eq!(config.creator.id, 12345);
        assert_eq!(config.inputs.len(), 1);
        let icons = config.inputs.get("icons").unwrap();
        assert_eq!(icons.include.to_string(), "icons/**/*.png");
        assert_eq!(icons.output_path.to_string_lossy(), "out/icons.luau");
    }

    fn run_async_test<F, T>(f: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }

    #[test]
    fn test_config_toml_parsing() {
        let dir = std::env::temp_dir().join("asphalt-test-config-toml");
        setup_temp_config(&dir, "asphalt.toml", minimal_config_toml());

        let config = run_async_test(Config::read_from(dir.clone())).unwrap();
        assert_valid_config(&config);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_json_parsing() {
        let dir = std::env::temp_dir().join("asphalt-test-config-json");
        setup_temp_config(&dir, "asphalt.json", minimal_config_json());

        let config = run_async_test(Config::read_from(dir.clone())).unwrap();
        assert_valid_config(&config);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_json5_parsing() {
        let dir = std::env::temp_dir().join("asphalt-test-config-json5");
        setup_temp_config(&dir, "asphalt.json5", minimal_config_json5());

        let config = run_async_test(Config::read_from(dir.clone())).unwrap();
        assert_valid_config(&config);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_jsonc_parsing() {
        let dir = std::env::temp_dir().join("asphalt-test-config-jsonc");
        setup_temp_config(&dir, "asphalt.jsonc", minimal_config_jsonc());

        let config = run_async_test(Config::read_from(dir.clone())).unwrap();
        assert_valid_config(&config);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_discovery_priority() {
        // Config discovery iterates CONFIG_FILES in order: jsonc, json, json5, toml
        // First file found wins. jsonc is first, so it should take priority over json and toml.
        let dir = std::env::temp_dir().join("asphalt-test-priority");
        setup_temp_config(
            &dir,
            "asphalt.json",
            r#"{"creator":{"type":"user","id":99999},"inputs":{"a":{"path":"*.png","output_path":"out/a.luau"}}}"#,
        );
        setup_temp_config(
            &dir,
            "asphalt.jsonc",
            r#"{"creator":{"type":"user","id":11111},"inputs":{"a":{"path":"*.png","output_path":"out/a.luau"}}}"#,
        );
        setup_temp_config(&dir, "asphalt.toml", minimal_config_toml());

        // jsonc is checked before json and toml, so jsonc config should be loaded
        let config = run_async_test(Config::read_from(dir.clone())).unwrap();
        assert_eq!(config.creator.id, 11111, "JSONC (first in CONFIG_FILES) should be loaded over JSON/TOML");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_missing_file_returns_error() {
        let dir = std::env::temp_dir().join("asphalt-test-missing");
        fs::create_dir_all(&dir).unwrap();

        let result = run_async_test(Config::read_from(dir.clone()));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No configuration file found"), "Error should mention missing config: {err}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_defaults() {
        let dir = std::env::temp_dir().join("asphalt-test-defaults");
        setup_temp_config(
            &dir,
            "asphalt.toml",
            r#"[creator]
type = "user"
id = 1

[inputs.x]
path = "*.png"
output_path = "out/x.luau"
"#,
        );

        let config = run_async_test(Config::read_from(dir.clone())).unwrap();
        // Default codegen values
        assert!(
            matches!(config.codegen.style, CodegenStyle::Flat),
            "Default codegen style should be Flat, got {:?}",
            config.codegen.style
        );
        assert!(!config.codegen.typescript, "Default typescript should be false");
        assert!(!config.codegen.strip_extensions, "Default strip_extensions should be false");
        assert!(!config.codegen.content, "Default content should be false");
        assert!(
            matches!(config.codegen.asset_naming_convention, AssetNamingConvention::Preserve),
            "Default asset naming convention should be Preserve"
        );
        // Input defaults
        let input = config.inputs.get("x").unwrap();
        assert!(input.bleed, "Default bleed should be true");
        assert!(input.warn_each_duplicate, "Default warn_each_duplicate should be true");
        assert!(input.pack.is_none(), "Default pack should be None");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_invalid_toml_returns_error() {
        let dir = std::env::temp_dir().join("asphalt-test-invalid");
        setup_temp_config(
            &dir,
            "asphalt.toml",
            r#"[creator]
type = "user"
id = "not-a-number"  # invalid type for id
"#,
        );

        let result = run_async_test(Config::read_from(dir.clone()));
        assert!(result.is_err(), "Invalid config should fail to parse");

        let _ = fs::remove_dir_all(&dir);
    }
}
