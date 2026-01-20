use fs_err::tokio as fs;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt::Display, path::Path, str::FromStr};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Hash(blake3::Hash);

impl Hash {
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        let hash = blake3::hash(bytes);
        Hash(hash)
    }

    pub async fn new_from_file(path: &Path) -> Result<Self, std::io::Error> {
        let bytes = fs::read(path).await?;
        Ok(Self::new_from_bytes(&bytes))
    }
}

impl Ord for Hash {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.as_bytes().cmp(other.0.as_bytes())
    }
}

impl PartialOrd for Hash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let hash = blake3::Hash::from_str(&str).map_err(serde::de::Error::custom)?;
        Ok(Hash(hash))
    }
}
