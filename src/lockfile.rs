use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LockfileEntry {
    pub hash: String,
    pub asset_id: u64,
}

pub const CURRENT_VERSION: u32 = 1;
pub const FILE_NAME: &str = "asphalt.lock.toml";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Lockfile {
    V0 {
        entries: HashMap<String, LockfileEntry>,
    },
    V1 {
        version: u32,
        inputs: HashMap<String, HashMap<String, LockfileEntry>>,
    },
}

impl Default for Lockfile {
    fn default() -> Self {
        Lockfile::V1 {
            version: CURRENT_VERSION,
            inputs: HashMap::new(),
        }
    }
}

impl Lockfile {
    pub async fn read() -> anyhow::Result<Self> {
        let content = fs::read_to_string(FILE_NAME).await;
        match content {
            Ok(content) => {
                let parsed = toml::from_str(&content)?;
                Ok(parsed)
            }
            Err(_) => Ok(Lockfile::default()),
        }
    }

    pub fn get(&self, input_name: &str, path: &Path) -> Option<&LockfileEntry> {
        match self {
            Lockfile::V0 { .. } => None,
            Lockfile::V1 { inputs, .. } => {
                let path_str = path.to_string_lossy().replace("\\", "/");
                inputs
                    .get(input_name)
                    .and_then(|assets| assets.get(&path_str))
            }
        }
    }

    pub fn insert(&mut self, input_name: &str, path: &Path, entry: LockfileEntry) {
        match self {
            Lockfile::V0 { .. } => {
                panic!("Cannot insert into version 0 lockfile!");
            }
            Lockfile::V1 { inputs, .. } => {
                let path_str = path.to_string_lossy().replace("\\", "/");
                let input_map = inputs.entry(input_name.to_string()).or_default();
                input_map.insert(path_str, entry);
            }
        }
    }

    pub async fn write(&self, filename: Option<&Path>) -> anyhow::Result<()> {
        match self {
            Lockfile::V0 { .. } => {
                anyhow::bail!("Cannot write out a version 0 lockfile!");
            }
            Lockfile::V1 { .. } => {
                let content = toml::to_string(self)?;
                fs::write(filename.unwrap_or(Path::new(FILE_NAME)), content).await?;
                Ok(())
            }
        }
    }

    pub fn get_all_if_v0(&self) -> anyhow::Result<HashMap<String, LockfileEntry>> {
        match self {
            Lockfile::V0 { entries } => Ok(entries.clone()),
            Lockfile::V1 { .. } => {
                bail!("Cannot flatten V1 lockfile into V0 format");
            }
        }
    }
}
