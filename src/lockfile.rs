use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use tokio::fs::{read_to_string, write};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub hash: String,
    pub asset_id: u64,
}

pub static FILE_NAME: &str = "asphalt.lock.toml";
pub static VERSION: u8 = 1;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LockFile {
    #[serde(default = "version_default")]
    pub version: u8,
    pub entries: BTreeMap<String, FileEntry>,
}

fn version_default() -> u8 {
    VERSION
}

impl LockFile {
    pub async fn read() -> anyhow::Result<Self> {
        let content = read_to_string(FILE_NAME).await;
        match content {
            Ok(content) => Ok(toml::from_str(&content)?),
            Err(_) => Ok(LockFile::default()),
        }
    }

    pub async fn write(&self, filename: &Path) -> anyhow::Result<()> {
        let content = toml::to_string(self)?;
        write(filename, content).await?;

        Ok(())
    }
}
