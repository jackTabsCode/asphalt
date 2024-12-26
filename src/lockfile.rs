use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use tokio::fs::{read_to_string, write};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub hash: String,
    pub asset_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LockFile {
    pub entries: BTreeMap<String, FileEntry>,
}

pub static FILE_NAME: &str = "asphalt.lock.toml";

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
