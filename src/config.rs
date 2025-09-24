use crate::glob::Glob;
use anyhow::Context;
use clap::ValueEnum;
use fs_err::tokio as fs;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub creator: Creator,

    #[serde(default)]
    pub codegen: Codegen,

    pub inputs: HashMap<String, Input>,
}

pub const FILE_NAME: &str = "asphalt.toml";

impl Config {
    pub async fn read() -> anyhow::Result<Config> {
        let config = fs::read_to_string(FILE_NAME)
            .await
            .context("Failed to read config file")?;
        let config: Config = toml::from_str(&config)?;

        Ok(config)
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Codegen {
    pub style: CodegenStyle,
    pub typescript: bool,
    pub strip_extensions: bool,
    pub content: bool,
}

#[derive(Debug, Deserialize, Clone, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum CreatorType {
    User,
    Group,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Creator {
    #[serde(rename = "type")]
    pub ty: CreatorType,
    pub id: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct Input {
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

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
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

#[derive(Debug, Deserialize, Clone, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum PackAlgorithm {
    MaxRects,
    Guillotine,
}

#[derive(Debug, Deserialize, Clone, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum PackSort {
    Area,
    MaxSide,
    Name,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CodegenStyle {
    #[default]
    Flat,
    Nested,
}
