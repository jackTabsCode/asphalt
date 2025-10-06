use crate::glob::Glob;
use anyhow::Context;
use clap::ValueEnum;
use fs_err::tokio as fs;
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
}

pub const CONFIG_FILES: &[&str] = &[
    "asphalt.json",
    "asphalt.json5",
    "asphalt.jsonc",
    "asphalt.toml",
];

impl Config {
    pub async fn read() -> anyhow::Result<Config> {
        // Try each config file in priority order
        for &file_name in CONFIG_FILES {
            if fs::metadata(file_name).await.is_ok() {
                let content = fs::read_to_string(file_name)
                    .await
                    .with_context(|| format!("Failed to read config file: {}", file_name))?;

                let config = match file_name {
                    name if name.ends_with(".json") => {
                        // Use fjson for lenient JSON parsing (supports trailing commas and comments)
                        let clean_json = fjson::to_json(&content).with_context(|| {
                            format!("Failed to parse JSON config file: {}", file_name)
                        })?;
                        serde_json::from_str(&clean_json).with_context(|| {
                            format!("Failed to deserialize JSON config: {}", file_name)
                        })?
                    }
                    name if name.ends_with(".json5") => {
                        json5::from_str(&content).with_context(|| {
                            format!("Failed to parse JSON5 config file: {}", file_name)
                        })?
                    }
                    name if name.ends_with(".jsonc") => {
                        // Use fjson for JSONC files (supports comments and trailing commas)
                        let clean_json = fjson::to_json(&content).with_context(|| {
                            format!("Failed to parse JSONC config file: {}", file_name)
                        })?;
                        serde_json::from_str(&clean_json).with_context(|| {
                            format!("Failed to deserialize JSONC config: {}", file_name)
                        })?
                    }
                    name if name.ends_with(".toml") => {
                        toml::from_str(&content).with_context(|| {
                            format!("Failed to parse TOML config file: {}", file_name)
                        })?
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Unsupported config file format: {}",
                            file_name
                        ));
                    }
                };

                log::info!("Loaded configuration from {}", file_name);
                return Ok(config);
            }
        }

        Err(anyhow::anyhow!(
            "No configuration file found. Please create one of: {}",
            CONFIG_FILES.join(", ")
        ))
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
    #[schemars(description = "Glob pattern to match asset files (e.g., 'assets/**/*.png')")]
    pub path: Glob,
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
    #[schemars(description = "Web assets that are already uploaded, mapped by path")]
    pub web: HashMap<String, WebAsset>,

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
    #[schemars(
        description = "Maximum number of atlas pages to generate (optional, unlimited by default)"
    )]
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
    Guillotine,
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
    #[schemars(description = "lowercase_with_underscores (e.g., 'my_input')")]
    SnakeCase,
    #[default]
    #[schemars(description = "firstWordLowerRestCapitalized (e.g., 'myInput') - default")]
    CamelCase,
    #[schemars(description = "AllWordsCapitalized (e.g., 'MyInput')")]
    PascalCase,
    #[schemars(description = "UPPERCASE_WITH_UNDERSCORES (e.g., 'MY_INPUT')")]
    ScreamingSnakeCase,
}

#[derive(Debug, Deserialize, Clone, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(description = "Naming convention for asset keys in generated code")]
#[allow(clippy::enum_variant_names)]
pub enum AssetNamingConvention {
    #[schemars(description = "lowercase_with_underscores (e.g., 'my_asset_name')")]
    SnakeCase,
    #[schemars(description = "firstWordLowerRestCapitalized (e.g., 'myAssetName')")]
    CamelCase,
    #[schemars(description = "AllWordsCapitalized (e.g., 'MyAssetName')")]
    PascalCase,
    #[schemars(description = "UPPERCASE_WITH_UNDERSCORES (e.g., 'MY_ASSET_NAME')")]
    ScreamingSnakeCase,
    #[schemars(description = "lowercase-with-hyphens (e.g., 'my-asset-name')")]
    KebabCase,
    #[default]
    #[schemars(
        description = "Preserve original name, quote if contains special characters - default"
    )]
    Preserve,
}
