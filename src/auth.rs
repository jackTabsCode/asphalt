use anyhow::bail;
use std::env;

pub struct Auth {
    pub api_key: String,
    pub cookie: Option<String>,
}

impl Auth {
    pub fn new(arg_key: Option<String>, auth_required: bool) -> anyhow::Result<Self> {
        let env_key = env::var("ASPHALT_API_KEY").ok();

        let cookie = rbx_cookie::get();

        let api_key = match arg_key.or(env_key) {
            Some(key) => key,
            None if auth_required => {
                bail!(err_str("API key"))
            }
            None => String::new(),
        };

        Ok(Self { api_key, cookie })
    }
}

fn err_str(ty: &str) -> String {
    format!(
        "A {ty} is required to use Asphalt. See the README for more information:\nhttps://github.com/jackTabsCode/asphalt?tab=readme-ov-file#authentication",
    )
}
