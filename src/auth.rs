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

        match (arg_key, env_key, key_required) {
            (Some(key), _, true) => Ok(Self {
                api_key: key,
                cookie,
            }),
            (_, Some(key), true) => Ok(Self {
                api_key: key,
                cookie,
            }),
            (None, None, false) => Ok(Self {
                api_key: String::new(), // Heh heh heh heh
                cookie,
            }),
            _ => bail!(
                "Either the API Key argument or ASPHALT_API_KEY variable must be set to \
                use Asphalt.\nAcquire one here: https://create.roblox.com/dashboard/credentials"
            ),
        }
    }
}
