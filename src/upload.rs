use anyhow::{bail, Context};
use log::{debug, warn};
use rbxcloud::rbx::{
    self,
    v1::assets::{
        create_asset_with_contents, get_operation, AssetCreation, AssetCreationContext,
        CreateAssetParamsWithContents, GetAssetOperationParams,
    },
};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::{
    asset::{Asset, AssetKind},
    config::Creator,
};

const ASSET_DESCRIPTION: &str = "Uploaded by Asphalt";

pub async fn upload_cloud(
    asset: &Asset,
    api_key: String,
    creator: &Creator,
) -> anyhow::Result<u64> {
    let display_name = asset.path.to_string_lossy().to_string();

    let params = CreateAssetParamsWithContents {
        contents: &asset.data,
        api_key: api_key.clone(),
        asset: AssetCreation {
            asset_type: asset.kind.clone().try_into()?,
            display_name,
            description: ASSET_DESCRIPTION.to_string(),
            creation_context: AssetCreationContext {
                creator: creator.clone().into(),
                expected_price: Some(0),
            },
        },
    };

    let op = create_asset_with_contents(&params).await?;
    let id = op
        .path
        .as_ref()
        .and_then(|p| p.strip_prefix("operations/"))
        .context("Path was invalid")?
        .to_string();

    let get_params = GetAssetOperationParams {
        api_key,
        operation_id: id,
    };

    let mut backoff = Duration::from_millis(10);
    loop {
        match get_operation(&get_params).await {
            Ok(op) if op.done.unwrap_or(false) => {
                if let Some(response) = op.response {
                    let id_str = response.asset_id;
                    let id = id_str.parse::<u64>().context("Asset ID wasn't a number")?;

                    return match asset.kind {
                        AssetKind::Decal(_) => get_image_id(id).await,
                        _ => Ok(id),
                    };
                }
            }
            Ok(_) => {
                debug!("Asset operation not done, retrying...");
            }
            Err(rbx::error::Error::HttpStatusError { code: 404, .. }) => {
                debug!("Asset not found, retrying...");
            }
            Err(rbx::error::Error::HttpStatusError { code: 429, .. }) => {
                warn!("Rate limited, retrying...");
            }
            Err(e) => bail!("Failed to GET asset: {:?}", e),
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(5));
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Roblox {
    item: Item,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Item {
    properties: Properties,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Properties {
    content: Content,
}

#[derive(Deserialize, Debug)]
struct Content {
    url: String,
}

async fn get_image_id(asset_id: u64) -> anyhow::Result<u64> {
    let client = Client::new();
    let url = format!("https://assetdelivery.roblox.com/v1/asset?id={}", asset_id);

    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to get image ID")?;

    let body = response.text().await?;
    let roblox: Roblox = serde_xml_rs::from_str(&body)?;

    roblox
        .item
        .properties
        .content
        .url
        .strip_prefix("http://www.roblox.com/asset/?id=")
        .unwrap()
        .parse::<u64>()
        .context("Asset ID wasn't a number")
}
