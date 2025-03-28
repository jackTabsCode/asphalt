use anyhow::bail;
use std::env;

pub struct Auth {
    pub api_key: String,
    pub cookie: Option<String>,
}

impl Auth {
    pub fn new(arg_key: Option<String>, key_required: bool) -> anyhow::Result<Self> {
        let env_key = env::var("ASPHALT_API_KEY").ok();

        let cookie = rbx_cookie::get();

        let api_key = match (arg_key, env_key) {
            (Some(key), _) => key,
            (None, Some(key)) => key,
            (None, None) => {
                if key_required {
                    bail!(
                        "Either the API Key argument or ASPHALT_API_KEY variable must be set to use Asphalt.\nAcquire one here: https://create.roblox.com/dashboard/credentials"
                    )
                } else {
                    String::new() // Heh heh heh heh
                }
            }
        };

        Ok(Self { api_key, cookie })
    }
}
