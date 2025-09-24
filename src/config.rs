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
    pub creator: Creator,

    #[serde(default)]
    pub codegen: Codegen,

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
                        let clean_json = fjson::to_json(&content)
                            .with_context(|| format!("Failed to parse JSON config file: {}", file_name))?;
                        serde_json::from_str(&clean_json)
                            .with_context(|| format!("Failed to deserialize JSON config: {}", file_name))?
                    },
                    name if name.ends_with(".json5") => {
                        json5::from_str(&content)
                            .with_context(|| format!("Failed to parse JSON5 config file: {}", file_name))?
                    },
                    name if name.ends_with(".jsonc") => {
                        // Use fjson for JSONC files (supports comments and trailing commas)
                        let clean_json = fjson::to_json(&content)
                            .with_context(|| format!("Failed to parse JSONC config file: {}", file_name))?;
                        serde_json::from_str(&clean_json)
                            .with_context(|| format!("Failed to deserialize JSONC config: {}", file_name))?
                    },
                    name if name.ends_with(".toml") => {
                        toml::from_str(&content)
                            .with_context(|| format!("Failed to parse TOML config file: {}", file_name))?
                    },
                    _ => {
                        return Err(anyhow::anyhow!("Unsupported config file format: {}", file_name));
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

#[derive(Debug, Deserialize, Clone, Default, JsonSchema)]
#[serde(default)]
#[schemars(description = "Code generation settings")]
pub struct Codegen {
    pub style: CodegenStyle,
    pub typescript: bool,
    pub strip_extensions: bool,
    pub content: bool,
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
    pub ty: CreatorType,
    pub id: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[schemars(description = "Input asset configuration")]
pub struct Input {
    #[schemars(with = "String")]
    pub path: Glob,
    pub output_path: PathBuf,
    pub pack: Option<PackOptions>,
    #[serde(default = "default_true")]
    pub bleed: bool,

    #[serde(default)]
    pub web: HashMap<String, WebAsset>,

    #[serde(default = "default_true")]
    pub warn_each_duplicate: bool,
}

#[derive(Debug, Deserialize, Clone, JsonSchema)]
#[schemars(description = "Web asset configuration")]
pub struct WebAsset {
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
    pub enabled: bool,
    #[serde(default = "default_pack_max_size")]
    pub max_size: (u32, u32),
    #[serde(default = "default_pack_power_of_two")]
    pub power_of_two: bool,
    #[serde(default = "default_pack_padding")]
    pub padding: u32,
    #[serde(default = "default_pack_extrude")]
    pub extrude: u32,
    pub allow_trim: bool,
    #[serde(default = "default_pack_algorithm")]
    pub algorithm: PackAlgorithm,
    pub page_limit: Option<u32>,
    #[serde(default = "default_pack_sort")]
    pub sort: PackSort,
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
