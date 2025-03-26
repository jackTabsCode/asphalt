use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockfileEntry {
    pub hash: String,
    pub asset_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Lockfile {
    entries: BTreeMap<String, LockfileEntry>,
}

pub const FILE_NAME: &str = "asphalt.lock.toml";

impl Lockfile {
    pub fn blank() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub async fn read() -> anyhow::Result<Self> {
        let content = fs::read_to_string(FILE_NAME).await;
        match content {
            Ok(content) => Ok(toml::from_str(&content)?),
            Err(_) => Ok(Lockfile::default()),
        }
    }

    fn format_path(input_name: &str, path: &Path) -> String {
        format!("{}/{}", input_name, path.display())
    }

    pub fn get(&self, input_name: String, path: &Path) -> Option<&LockfileEntry> {
        self.entries.get(&Lockfile::format_path(&input_name, path))
    }

    pub fn insert(&mut self, input_name: String, path: &Path, entry: LockfileEntry) {
        self.entries
            .insert(Lockfile::format_path(&input_name, path), entry);
    }

    pub async fn write(&self, filename: Option<&Path>) -> anyhow::Result<()> {
        let content = toml::to_string(self)?;
        fs::write(filename.unwrap_or(Path::new(FILE_NAME)), content).await?;

        Ok(())
    }
}
