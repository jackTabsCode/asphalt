use anyhow::bail;
use fs_err::tokio as fs;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

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
        entries: BTreeMap<String, LockfileEntry>,
    },
    V1 {
        version: u32,
        inputs: BTreeMap<String, BTreeMap<String, LockfileEntry>>,
    },
    V2 {
        version: u32,
        inputs: BTreeMap<String, BTreeMap<String, LockfileEntry>>,
    },
}

impl Default for Lockfile {
    fn default() -> Self {
        Lockfile::V1 {
            version: CURRENT_VERSION,
            inputs: BTreeMap::new(),
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

    pub fn get(&self, input_name: &str, hash: &str) -> Option<&LockfileEntry> {
        match self {
            Lockfile::V0 { .. } => unreachable!(),
            Lockfile::V1 { .. } => unreachable!(),
            Lockfile::V2 { inputs, .. } => {
                inputs.get(input_name).and_then(|assets| assets.get(hash))
            }
        }
    }

    pub fn insert(&mut self, input_name: &str, hash: &str, entry: LockfileEntry) {
        match self {
            Lockfile::V0 { .. } => unreachable!(),
            Lockfile::V1 { .. } => unreachable!(),
            Lockfile::V2 { inputs, .. } => {
                let input_map = inputs.entry(input_name.to_string()).or_default();
                input_map.insert(hash.to_string(), entry);
            }
        }
    }

    pub async fn write(&self, filename: Option<&Path>) -> anyhow::Result<()> {
        match self {
            Lockfile::V0 { .. } => unreachable!(),
            Lockfile::V1 { .. } => unreachable!(),
            Lockfile::V2 { .. } => {
                let content = toml::to_string(self)?;
                fs::write(filename.unwrap_or(Path::new(FILE_NAME)), content).await?;
                Ok(())
            }
        }
    }
}
