use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StyleType {
    Flat,
    Nested,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CreatorType {
    User,
    Group,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Creator {
    #[serde(rename = "type")]
    pub creator_type: CreatorType,
    pub id: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExistingAsset {
    pub id: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CodegenConfig {
    pub output_name: Option<String>,
    pub typescript: Option<bool>,
    pub luau: Option<bool>,
    pub style: Option<StyleType>,
    pub strip_extension: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SyncConfig {
    pub asset_dir: String,
    pub write_dir: String,
    pub creator: Creator,
    pub codegen: CodegenConfig,
    pub existing: Option<HashMap<String, ExistingAsset>>,
}
