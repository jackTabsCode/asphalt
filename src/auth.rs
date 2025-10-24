use anyhow::bail;
use std::env;

pub struct Auth {
    pub api_key: Option<String>,
}

impl Auth {
    pub fn new(arg_key: Option<String>, auth_required: bool) -> anyhow::Result<Self> {
        let env_key = env::var("ASPHALT_API_KEY").ok();

        let api_key = match arg_key.or(env_key) {
            Some(key) => Some(key),
            None if auth_required => {
                bail!(err_str("API key"))
            }
            None => None,
        };

        Ok(Self { api_key })
    }
}

fn err_str(ty: &str) -> String {
    format!(
        "A {ty} is required to use Asphalt. See the README for more information:\nhttps://github.com/jackTabsCode/asphalt?tab=readme-ov-file#authentication",
    )
}
