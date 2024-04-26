use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
pub struct Config {
    pub asset_dir: String,
    pub write_dir: String,
    pub creator: Creator,
    pub output_name: Option<String>,
    pub typescript: Option<bool>,
    pub luau: Option<bool>,
    pub style: Option<StyleType>,

    pub existing: Option<HashMap<String, ExistingAsset>>,
}
