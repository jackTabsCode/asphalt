use std::path::PathBuf;

use crate::{
    args::{Args, AssetCreatorGroup},
    LockFile,
};
use rbxcloud::rbx::v1::assets::{AssetCreator, AssetGroupCreator, AssetUserCreator};
use tokio::fs::{create_dir_all, read_to_string};

pub struct State {
    pub asset_dir: PathBuf,
    pub write_dir: PathBuf,

    pub api_key: String,
    pub creator: AssetCreator,
    pub typescript: bool,
    pub output_name: String,
    pub lua_extension: String,

    pub existing_lockfile: LockFile,
    pub new_lockfile: LockFile,
}

impl State {
    pub async fn new(args: Args) -> Self {
        let api_key: String = args
            .api_key
            .unwrap_or_else(|| std::env::var("ASPHALT_API_KEY").expect("no API key provided"));

        let asset_creator: AssetCreator = match args.creator {
            AssetCreatorGroup {
                user_id: Some(user_id),
                group_id: None,
            } => AssetCreator::User(AssetUserCreator {
                user_id: user_id.to_string(),
            }),
            AssetCreatorGroup {
                user_id: None,
                group_id: Some(group_id),
            } => AssetCreator::Group(AssetGroupCreator {
                group_id: group_id.to_string(),
            }),
            _ => panic!("either user_id or group_id must be provided"),
        };

        let asset_dir = PathBuf::from(args.asset_dir);

        create_dir_all(&args.write_dir)
            .await
            .expect("can't create write directory");
        let write_dir = PathBuf::from(args.write_dir);

        let output_name = args.output_name.unwrap_or("assets".to_string());

        let lua_extension = String::from(if args.luau { "luau" } else { "lua" });

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
            creator: asset_creator,
            typescript: args.typescript,
            output_name,
            lua_extension,
            existing_lockfile,
            new_lockfile,
        }
    }
}
