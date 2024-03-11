use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub hash: String,
    pub asset_id: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LockFile {
    pub entries: BTreeMap<String, FileEntry>,
}
