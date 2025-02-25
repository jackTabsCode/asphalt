use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpriteInfo {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub spritesheet: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub hash: Option<String>,
    pub asset_id: u64,
    pub sprite: Option<SpriteInfo>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LockFile {
    pub entries: BTreeMap<String, FileEntry>,
}

pub static FILE_NAME: &str = "asphalt.lock.toml";

impl LockFile {
    pub async fn read() -> anyhow::Result<Self> {
        let content = fs::read_to_string(FILE_NAME).await;
        match content {
            Ok(content) => Ok(toml::from_str(&content)?),
            Err(_) => Ok(LockFile::default()),
        }
    }

    pub async fn write(&self, filename: &Path) -> anyhow::Result<()> {
        let content = toml::to_string(self)?;
        fs::write(filename, content).await?;

        Ok(())
    }
}
