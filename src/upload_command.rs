use crate::{
    asset::{Asset, AssetKind, ModelKind},
    auth::Auth,
    cli::UploadArgs,
    config::Creator,
    upload::{upload_animation, upload_cloud},
};
use anyhow::bail;
use fs_err::tokio as fs;
use resvg::usvg::fontdb::Database;
use std::{path::PathBuf, sync::Arc};

pub async fn upload(args: UploadArgs) -> anyhow::Result<()> {
    let path = PathBuf::from(&args.path);
    let data = fs::read(&path).await?;

    let mut asset = Asset::new(path, data)?;

    let mut font_db = Database::new();
    font_db.load_system_fonts();

    asset.process(Arc::new(font_db), args.bleed).await?;

    let creator = Creator {
        ty: args.creator_type,
        id: args.creator_id,
    };

    let auth = Auth::new(args.api_key, true)?;

    let client = reqwest::Client::new();

    let asset_id = match asset.kind {
        AssetKind::Decal(_) | AssetKind::Audio(_) | AssetKind::Model(ModelKind::Model) => {
            upload_cloud(client, &asset, auth.api_key, &creator, auth.cookie.clone()).await?
        }
        AssetKind::Model(ModelKind::Animation(_)) => {
            let Some(cookie) = auth.cookie.clone() else {
                bail!("Cookie required for uploading animations")
            };

            upload_animation(client, &asset, cookie, None, &creator)
                .await?
                .asset_id
        }
    };
    if args.link {
        println!("https://create.roblox.com/store/asset/{}", asset_id);
    } else {
        println!("{}", asset_id);
    }

    Ok(())
}
