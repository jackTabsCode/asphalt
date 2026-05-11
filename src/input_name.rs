use crate::identifier;
use anyhow::bail;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{fmt::Display, ops::Deref};

#[derive(
    Debug, Deserialize, Serialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, JsonSchema,
)]
pub struct InputName(#[serde(deserialize_with = "deserialize_input_name")] String);

const ERROR: &str =
    "invalid identifier (must use letters, numbers, or '_', and cannot start with a number)";

impl InputName {
    pub fn new(name: String) -> anyhow::Result<Self> {
        if identifier::is_valid(&name) {
            Ok(Self(name))
        } else {
            bail!(ERROR)
        }
    }
}

impl Deref for InputName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for InputName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

fn deserialize_input_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if identifier::is_valid(&s) {
        Ok(s)
    } else {
        Err(de::Error::custom(ERROR))
    }
}
