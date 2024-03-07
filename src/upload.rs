use std::{path::PathBuf, time::Duration};

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

static CONTENT_URL_PREFIX: &str = "http://www.roblox.com/asset/?id=";

async fn get_image_id(asset_id: u64) -> u64 {
    let client = Client::new();
    let url = format!("https://assetdelivery.roblox.com/v1/asset?id={}", asset_id);

    let response = client
        .get(url)
        .send()
        .await
        .expect("failed to get image id");
    let body = response
        .text()
        .await
        .expect("failed to parse request body to text");

    let roblox: Roblox =
        from_str(&body).expect("failed to parse request body to Roblox XML format");

    let id_str = roblox
        .item
        .properties
        .content
        .url
        .strip_prefix(CONTENT_URL_PREFIX)
        .unwrap()
        .to_string();

    id_str.parse::<u64>().unwrap()
}

pub async fn upload_asset(
    path: PathBuf,
    asset_type: AssetType,
    api_key: String,
    creator: AssetCreator,
) -> u64 {
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

    loop {
        match get_asset(&create_params).await {
            Ok(asset_operation) => {
                if let Some(done) = asset_operation.done {
                    if let Some(response) = asset_operation.response {
                        if done {
                            let id_str = response.asset_id;
                            let id = id_str.parse::<u64>().unwrap();

                            match asset_type {
                                AssetType::DecalPng
                                | AssetType::DecalJpeg
                                | AssetType::DecalBmp => return get_image_id(id).await,
                                _ => return id,
                            }
                        }
                    } else {
                        panic!("no response from get_asset, your file might be empty?");
                    }
                }
            }
            Err(e) => {
                if let Error::HttpStatusError { code, msg } = e {
                    if code != 404 {
                        panic!("{}: {}", code, msg);
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
