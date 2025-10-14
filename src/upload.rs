use crate::{asset::Asset, auth::Auth, cli::UploadArgs, config::Creator, web_api::WebApiClient};
use fs_err::tokio as fs;
use relative_path::PathExt;
use resvg::usvg::fontdb::Database;
use std::{path::PathBuf, sync::Arc};

pub async fn upload(args: UploadArgs) -> anyhow::Result<()> {
    let path = PathBuf::from(&args.path);
    let data = fs::read(&path).await?;

    let mut asset = Asset::new(path.relative_to(".")?, data)?;

    let mut font_db = Database::new();
    font_db.load_system_fonts();

    asset.process(Arc::new(font_db), args.bleed).await?;

    let creator = Creator {
        ty: args.creator_type,
        id: args.creator_id,
    };
    let auth = Auth::new(args.api_key, true)?;

    let client = WebApiClient::new(auth, creator, args.expected_price);

    let asset_id = client.upload(&asset).await?;

    if args.link {
        println!("https://create.roblox.com/store/asset/{asset_id}");
    } else {
        println!("{asset_id}");
    }

    Ok(())
}
