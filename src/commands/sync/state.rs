use super::config::{CodegenStyle, CreatorType, ExistingAsset, SyncConfig};
use crate::{
    cli::{SyncArgs, SyncTarget},
    LockFile,
};
use anyhow::Context;
use cookie::Cookie;
use globset::{Glob, GlobSet, GlobSetBuilder};
use log::debug;
use rbxcloud::rbx::v1::assets::{AssetCreator, AssetGroupCreator, AssetUserCreator};
use resvg::usvg::fontdb::Database;
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::fs;

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

fn get_cookie(arg_cookie: Option<String>) -> Option<String> {
    let env_cookie = env::var("ASPHALT_COOKIE").ok();
    let cookie_str = arg_cookie.or(env_cookie).or(rbx_cookie::get_value());

    cookie_str.map(|cookie| {
        Cookie::build(".ROBLOSECURITY", cookie)
            .domain(".roblox.com")
            .finish()
            .to_string()
    })
}

pub struct SyncState {
    pub asset_dir: PathBuf,
    pub write_dir: PathBuf,
    pub exclude_assets_matcher: GlobSet,
    pub spritesheet_matcher: GlobSet,

    pub api_key: String,
    pub cookie: Option<String>,
    pub target: SyncTarget,
    pub dry_run: bool,
    pub csrf: Option<String>,

    pub creator: AssetCreator,

    pub typescript: bool,
    pub output_name: String,
    pub style: CodegenStyle,
    pub strip_extension: bool,

    pub fontdb: Arc<Database>,

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
        let cookie = get_cookie(args.cookie);
        let target = args.target.unwrap_or(SyncTarget::Cloud);

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

        let _ = fs::create_dir_all(&config.write_dir)
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

        let strip_extension = config.codegen.strip_extension.unwrap_or(false);

        let mut font_db = Database::new();
        font_db.load_system_fonts();

        let new_lockfile: LockFile = Default::default();

        let mut exclude_assets_matcher_builder = GlobSetBuilder::new();
        for glob in config.exclude_assets {
            let glob = Glob::new(&glob)?;
            exclude_assets_matcher_builder.add(glob);
        }
        let exclude_assets_matcher = exclude_assets_matcher_builder.build()?;

        let mut spritesheet_matcher_builder = GlobSetBuilder::new();
        for glob in config.spritesheets {
            let glob = Glob::new(&glob)?;
            spritesheet_matcher_builder.add(glob);
        }

        let spritesheet_matcher = spritesheet_matcher_builder.build()?;

        Ok(Self {
            asset_dir,
            write_dir,
            exclude_assets_matcher,
            spritesheet_matcher,
            api_key,
            creator,
            typescript,
            output_name,
            style,
            strip_extension,
            fontdb: Arc::new(font_db),
            existing_lockfile,
            new_lockfile,
            existing: config.existing,
            cookie,
            target,
            dry_run: args.dry_run,
            csrf: None,
        })
    }

    pub fn update_csrf(&mut self, csrf: Option<String>) {
        self.csrf = csrf;
    }

    pub fn is_in_spritesheet(&self, path: &str) -> bool {
        let asset_dir_str = self.asset_dir.to_str().unwrap();

        let relative_path = if path.starts_with(asset_dir_str) {
            path.strip_prefix(asset_dir_str)
                .unwrap_or(path)
                .trim_start_matches('/')
        } else {
            path
        };

        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext {
            Some(ext) if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "tga" | "svg") => {
                if self.spritesheet_matcher.is_match(relative_path) {
                    debug!("Path '{}' matches a spritesheet glob", relative_path);
                    return true;
                }
            }
            _ => {}
        }

        false
    }
}
