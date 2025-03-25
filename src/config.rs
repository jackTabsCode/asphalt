use anyhow::Context;
use clap::ValueEnum;
use rbxcloud::rbx::v1::assets::{AssetCreator, AssetGroupCreator, AssetUserCreator};
use serde::Deserialize;
use std::path::PathBuf;

use crate::glob::Glob;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub creator: Creator,
    pub codegen: Codegen,
    pub inputs: Vec<Input>,
}

pub const FILE_NAME: &str = "asphalt.toml";

impl Config {
    pub fn read() -> anyhow::Result<Config> {
        let config = std::fs::read_to_string(FILE_NAME).context("Failed to read config file")?;
        let config: Config = toml::from_str(&config)?;
        Ok(config)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Codegen {
    style: CodegenStyle,
    #[serde(default)]
    typescript: bool,
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

impl From<Creator> for AssetCreator {
    fn from(creator: Creator) -> AssetCreator {
        match creator.ty {
            CreatorType::User => AssetCreator::User(AssetUserCreator {
                user_id: creator.id.to_string(),
            }),
            CreatorType::Group => AssetCreator::Group(AssetGroupCreator {
                group_id: creator.id.to_string(),
            }),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct Input {
    pub name: String,
    pub path: Glob,
    pub output_path: PathBuf,
    // pub pack: Option<PackOptions>,
    #[serde(default = "default_true")]
    pub bleed: bool,
}

// fn default_pack_size() -> u32 {
//     1024
// }

// #[derive(Debug, Deserialize, Clone)]
// pub struct PackOptions {
//     #[serde(default = "default_pack_size")]
//     size: u32,
// }

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "snake_case")]
enum CodegenStyle {
    #[default]
    Flat,
    Nested,
}
