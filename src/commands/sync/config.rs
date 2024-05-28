use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
};
use tokio::fs::{read_to_string, write};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CodegenStyle {
    Flat,
    Nested,
}

impl Display for CodegenStyle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CodegenStyle::Flat => write!(f, "Flat"),
            CodegenStyle::Nested => write!(f, "Nested"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CreatorType {
    User,
    Group,
}

impl Display for CreatorType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CreatorType::User => write!(f, "User"),
            CreatorType::Group => write!(f, "Group"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Creator {
    #[serde(rename = "type")]
    pub creator_type: CreatorType,
    pub id: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExistingAsset {
    pub id: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CodegenConfig {
    pub output_name: Option<String>,
    pub typescript: Option<bool>,
    pub luau: Option<bool>,
    pub style: Option<CodegenStyle>,
    pub strip_extension: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SyncConfig {
    pub asset_dir: String,
    pub write_dir: String,
    pub creator: Creator,
    pub codegen: CodegenConfig,
    pub existing: Option<HashMap<String, ExistingAsset>>,
}

static FILE_NAME: &str = "asphalt.toml";

impl SyncConfig {
    pub async fn read() -> anyhow::Result<Self> {
        let content = read_to_string(FILE_NAME)
            .await
            .context("Failed to read config. Did you create it?")?;
        toml::from_str(&content).context("Failed to parse config")
    }

    pub async fn write(&self) -> anyhow::Result<()> {
        let content = toml::to_string(self)?;
        write(FILE_NAME, content)
            .await
            .context("Failed to write config")
    }
}
