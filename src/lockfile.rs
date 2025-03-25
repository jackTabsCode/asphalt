use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use tokio::fs::{read_to_string, write};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockfileEntry {
    pub hash: String,
    pub asset_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Lockfile {
    pub entries: BTreeMap<String, LockfileEntry>,
}

pub const FILE_NAME: &str = "asphalt.lock.toml";

impl Lockfile {
    pub async fn read() -> anyhow::Result<Self> {
        let content = read_to_string(FILE_NAME).await;
        match content {
            Ok(content) => Ok(toml::from_str(&content)?),
            Err(_) => Ok(Lockfile::default()),
        }
    }

    pub async fn write(&self, filename: Option<&Path>) -> anyhow::Result<()> {
        let content = toml::to_string(self)?;
        write(filename.unwrap_or(Path::new(FILE_NAME)), content).await?;

        Ok(())
    }
}
