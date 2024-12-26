use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tokio::fs::{read_to_string, write};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub hash: String,
    pub asset_id: u64,
}

static FILE_NAME: &str = "asphalt.lock.toml";
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

    pub async fn write(&mut self) -> anyhow::Result<()> {
        self.version = VERSION;
        let content = toml::to_string(self)?;
        write(FILE_NAME, content).await?;

        Ok(())
    }
}
