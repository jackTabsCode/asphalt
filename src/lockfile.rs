use anyhow::{Context, bail};
use blake3::Hasher;
use fs_err::tokio as fs;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct OldLockfileEntry {
    pub hash: String,
    pub asset_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LockfileEntry {
    pub asset_id: u64,
}

pub const FILE_NAME: &str = "asphalt.lock.toml";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LockfileV0 {
    entries: BTreeMap<PathBuf, OldLockfileEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LockfileV1 {
    version: u32,
    inputs: BTreeMap<String, BTreeMap<PathBuf, OldLockfileEntry>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LockfileV2 {
    version: u32,
    inputs: BTreeMap<String, BTreeMap<String, LockfileEntry>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Lockfile {
    V0(LockfileV0),
    V1(LockfileV1),
    V2(LockfileV2),
}

impl Default for Lockfile {
    fn default() -> Self {
        Lockfile::V2(LockfileV2 {
            version: 2,
            inputs: BTreeMap::new(),
        })
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
            Lockfile::V0(_) => unreachable!(),
            Lockfile::V1(_) => unreachable!(),
            Lockfile::V2(lockfile) => lockfile
                .inputs
                .get(input_name)
                .and_then(|assets| assets.get(hash)),
        }
    }

    pub fn insert(&mut self, input_name: &str, hash: &str, entry: LockfileEntry) {
        match self {
            Lockfile::V0(_) => unreachable!(),
            Lockfile::V1(_) => unreachable!(),
            Lockfile::V2(lockfile) => {
                let input_map = lockfile.inputs.entry(input_name.to_string()).or_default();
                input_map.insert(hash.to_string(), entry);
            }
        }
    }

    pub async fn write(&self, filename: Option<&Path>) -> anyhow::Result<()> {
        match self {
            Lockfile::V0(_) => unreachable!(),
            Lockfile::V1(_) => unreachable!(),
            Lockfile::V2(_) => {
                let content = toml::to_string(self)?;
                fs::write(filename.unwrap_or(Path::new(FILE_NAME)), content).await?;
                Ok(())
            }
        }
    }

    pub async fn migrate(&mut self, input_name: Option<String>) -> anyhow::Result<()> {
        *self = match (&self, input_name) {
            (Lockfile::V0(lockfile), Some(input_name)) => {
                migrate_from_v0(lockfile, &input_name).await?
            }
            (Lockfile::V0(_), None) => {
                bail!("An input name must be passed in order to migrate from v0 to v1")
            }
            (Lockfile::V1(lockfile), _) => migrate_from_v1(lockfile),
            (Lockfile::V2(_), _) => bail!("Your lockfile is already up to date"),
        };

        Ok(())
    }

    pub fn is_up_to_date(&self) -> bool {
        match self {
            Lockfile::V0(_) => false,
            Lockfile::V1(_) => false,
            Lockfile::V2(_) => true,
        }
    }
}

fn migrate_from_v1(lockfile: &LockfileV1) -> Lockfile {
    let mut new_lockfile = Lockfile::default();

    for (input_name, entries) in &lockfile.inputs {
        for entry in entries.values() {
            new_lockfile.insert(
                input_name,
                &entry.hash,
                LockfileEntry {
                    asset_id: entry.asset_id,
                },
            )
        }
    }

    new_lockfile
}

async fn migrate_from_v0(lockfile: &LockfileV0, input_name: &str) -> anyhow::Result<Lockfile> {
    let mut new_lockfile = Lockfile::default();

    for (path, entry) in &lockfile.entries {
        let new_hash = read_and_hash(path)
            .await
            .context(format!("Failed to hash {}", path.display()))?;

        new_lockfile.insert(
            input_name,
            &new_hash,
            LockfileEntry {
                asset_id: entry.asset_id,
            },
        )
    }

    Ok(new_lockfile)
}

async fn read_and_hash(path: &Path) -> anyhow::Result<String> {
    let file = fs::read(path).await?;

    let mut hasher = Hasher::new();
    hasher.update(&file);
    Ok(hasher.finalize().to_string())
}
