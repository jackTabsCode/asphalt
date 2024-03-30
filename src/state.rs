use std::path::PathBuf;

use crate::{
    args::Args,
    config::{Config, CreatorType},
    LockFile,
};
use rbxcloud::rbx::v1::assets::{AssetCreator, AssetGroupCreator, AssetUserCreator};
use resvg::usvg::fontdb::Database;
use tokio::fs::{create_dir_all, read_to_string};

fn add_trailing_slash(path: &str) -> String {
    if !path.ends_with('/') {
        return format!("{}/", path);
    }

    path.to_string()
}

pub struct State {
    pub asset_dir: PathBuf,
    pub write_dir: PathBuf,

    pub api_key: String,
    pub creator: AssetCreator,
    pub typescript: bool,
    pub output_name: String,
    pub lua_extension: String,

    pub font_db: Database,

    pub existing_lockfile: LockFile,
    pub new_lockfile: LockFile,
}

impl State {
    pub async fn new(args: Args, config: &Config) -> Self {
        let api_key: String = args
            .api_key
            .unwrap_or_else(|| std::env::var("ASPHALT_API_KEY").expect("No API key provided"));

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

        create_dir_all(&config.write_dir)
            .await
            .expect("Failed to create write directory");
        let write_dir = add_trailing_slash(&config.write_dir);
        let write_dir = PathBuf::from(write_dir);

        let output_name = config
            .output_name
            .as_ref()
            .unwrap_or(&"assets".to_string())
            .to_string();

        let typescript = config.typescript.unwrap_or(false);

        let lua_extension = String::from(if config.luau.unwrap_or(false) {
            "luau"
        } else {
            "lua"
        });

        let mut font_db = Database::new();
        font_db.load_system_fonts();

        let existing_lockfile: LockFile = toml::from_str(
            &read_to_string("asphalt.lock.toml")
                .await
                .unwrap_or_default(),
        )
        .unwrap_or_default();

        let new_lockfile: LockFile = Default::default();

        Self {
            asset_dir,
            write_dir,
            api_key,
            creator,
            typescript,
            output_name,
            lua_extension,
            font_db,
            existing_lockfile,
            new_lockfile,
        }
    }
}
