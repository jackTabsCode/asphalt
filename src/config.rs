use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CreatorType {
    #[default]
    User,
    Group,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Creator {
    #[serde(rename = "type")]
    pub creator_type: CreatorType,
    pub id: u64,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Config {
    pub asset_dir: String,
    pub write_dir: String,
    pub creator: Creator,
    pub output_name: Option<String>,
    pub typescript: Option<bool>,
    pub luau: Option<bool>,
}
