use super::config::{CodegenStyle, CreatorType, ExistingAsset, SyncConfig};
use crate::{cli::SyncArgs, LockFile};
use anyhow::Context;
use cookie::Cookie;
use rbxcloud::rbx::v1::assets::{AssetCreator, AssetGroupCreator, AssetUserCreator};
use resvg::usvg::fontdb::Database;
use std::{collections::HashMap, env, path::PathBuf};
use tokio::fs::create_dir_all;

fn add_trailing_slash(path: &str) -> String {
    if !path.ends_with('/') {
        return format!("{}/", path);
    }

    path.to_string()
}

fn get_api_key(arg_key: Option<String>) -> anyhow::Result<String> {
    let env_key = env::var("ASPHALT_API_KEY");

    match arg_key {
        Some(key) => Ok(key),
        None => env_key.context("No API key provided"),
    }
}

fn get_cookie(arg_cookie: Option<String>) -> anyhow::Result<Option<String>> {
    let env_cookie = env::var("ASPHALT_COOKIE").ok();
    let cookie_str = arg_cookie.or(env_cookie);

    if let Some(cookie) = cookie_str {
        Ok(Some(
            Cookie::build(".ROBLOSECURITY", cookie)
                .domain(".roblox.com")
                .finish()
                .to_string(),
        ))
    } else {
        Ok(None)
    }
}

pub struct SyncState {
    pub asset_dir: PathBuf,
    pub write_dir: PathBuf,

    pub api_key: String,
    pub cookie: Option<String>,
    pub csrf: Option<String>,

    pub creator: AssetCreator,

    pub typescript: bool,
    pub output_name: String,
    pub lua_extension: String,
    pub style: CodegenStyle,
    pub strip_extension: bool,

    pub font_db: Database,

    pub existing_lockfile: LockFile,
    pub new_lockfile: LockFile,

    pub existing: HashMap<String, ExistingAsset>,
}

impl SyncState {
    pub async fn new(
        args: SyncArgs,
        config: SyncConfig,
        existing_lockfile: LockFile,
    ) -> anyhow::Result<Self> {
        let api_key = get_api_key(args.api_key)?;
        let cookie = get_cookie(args.cookie)?;

        let creator: AssetCreator = match config.creator.creator_type {
            CreatorType::User => AssetCreator::User(AssetUserCreator {
                user_id: config.creator.id.to_string(),
            }),
            CreatorType::Group => AssetCreator::Group(AssetGroupCreator {
                group_id: config.creator.id.to_string(),
            }),
        };

        let asset_dir = add_trailing_slash(&config.asset_dir);
        let asset_dir = PathBuf::from(asset_dir);

        let _ = create_dir_all(&config.write_dir)
            .await
            .context("Failed to create write directory");
        let write_dir = add_trailing_slash(&config.write_dir);
        let write_dir = PathBuf::from(write_dir);

        let output_name = config
            .codegen
            .output_name
            .as_ref()
            .unwrap_or(&"assets".to_string())
            .to_string();

        let typescript = config.codegen.typescript.unwrap_or(false);
        let style = config.codegen.style.unwrap_or(CodegenStyle::Flat);

        let lua_extension = String::from(if config.codegen.luau.unwrap_or(false) {
            "luau"
        } else {
            "lua"
        });

        let strip_extension = config.codegen.strip_extension.unwrap_or(false);

        let mut font_db = Database::new();
        font_db.load_system_fonts();

        let new_lockfile: LockFile = Default::default();

        let manual = config.existing.unwrap_or_default();

        Ok(Self {
            asset_dir,
            write_dir,
            api_key,
            creator,
            typescript,
            output_name,
            lua_extension,
            style,
            strip_extension,
            font_db,
            existing_lockfile,
            new_lockfile,
            existing: manual,
            cookie,
            csrf: None,
        })
    }

    pub fn update_csrf(&mut self, csrf: Option<String>) {
        self.csrf = csrf;
    }
}
