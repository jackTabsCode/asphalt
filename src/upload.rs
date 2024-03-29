use std::{path::PathBuf, time::Duration};

use anyhow::{bail, Context};
use rbxcloud::rbx::assets::{
    create_asset, get_asset, AssetCreation, AssetCreationContext, AssetCreator, AssetType,
    CreateAssetParams, GetAssetParams,
};
use rbxcloud::rbx::error::Error;
use reqwest::Client;
use serde::Deserialize;
use serde_xml_rs::from_str;

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

    let body = response
        .text()
        .await
        .context("Failed to parse request body to text")?;

    let roblox: Roblox =
        from_str(&body).context("Failed to parse request body to Roblox XML format")?;

    let id_str = roblox
        .item
        .properties
        .content
        .url
        .strip_prefix("http://www.roblox.com/asset/?id=")
        .context("Failed to strip Roblox URL prefix")?
        .to_string();

    id_str.parse::<u64>().context("Failed to parse image ID")
}

pub async fn upload_asset(
    path: PathBuf,
    asset_type: AssetType,
    api_key: String,
    creator: AssetCreator,
) -> anyhow::Result<u64> {
    let path_str = path.to_str().unwrap();

    let create_params = CreateAssetParams {
        api_key: api_key.clone(),
        filepath: path_str.to_string(),
        asset: AssetCreation {
            asset_type,
            display_name: path_str.to_string(),
            creation_context: AssetCreationContext {
                creator,
                expected_price: None,
            },
            description: "Uploaded by Asphalt".to_string(),
        },
    };
    let operation = create_asset(&create_params).await.unwrap();

    let id = operation
        .path
        .unwrap()
        .strip_prefix("operations/")
        .unwrap()
        .to_string();

    let create_params = GetAssetParams {
        api_key,
        operation_id: id,
    };

    let mut backoff = Duration::from_millis(100);
    loop {
        match get_asset(&create_params).await {
            Ok(asset_operation) if asset_operation.done.unwrap_or(false) => {
                if let Some(response) = asset_operation.response {
                    let id_str = response.asset_id;
                    let id = id_str.parse::<u64>().expect("asset ID must be a u64");

                    return match asset_type {
                        AssetType::DecalPng
                        | AssetType::DecalJpeg
                        | AssetType::DecalBmp
                        | AssetType::DecalTga => get_image_id(id).await,
                        _ => Ok(id),
                    };
                }
            }
            Ok(_) => {}
            Err(Error::HttpStatusError { code: 404, .. }) => {}
            Err(e) => bail!("failed to get asset: {:?}", e),
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(10));
    }
}
