use anyhow::{bail, Context};
use log::{debug, warn};
use rbxcloud::rbx::error::Error;
use rbxcloud::rbx::v1::assets::{
    create_asset_with_contents, get_asset, AssetCreation, AssetCreationContext, AssetCreator,
    AssetType, CreateAssetParamsWithContents, GetAssetParams,
};
use reqwest::Client;
use serde::Deserialize;
use serde_xml_rs::from_str;
use std::time::Duration;
use tokio::time::sleep;

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

const ASSET_DESCRIPTION: &str = "Uploaded by Asphalt";

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

pub async fn upload_cloud_asset(
    contents: Vec<u8>,
    display_name: String,
    asset_type: AssetType,
    api_key: String,
    creator: AssetCreator,
) -> anyhow::Result<u64> {
    let create_params = CreateAssetParamsWithContents {
        contents: &contents,
        api_key: api_key.clone(),
        asset: AssetCreation {
            asset_type,
            display_name,
            creation_context: AssetCreationContext {
                creator,
                expected_price: None,
            },
            description: ASSET_DESCRIPTION.to_string(),
        },
    };
    let operation = create_asset_with_contents(&create_params)
        .await
        .context("Failed to create asset")?;

    let id = operation
        .path
        .context("The operation had no path")?
        .strip_prefix("operations/")
        .context("The operation path was not prefixed with 'operations/'")?
        .to_string();

    let get_params = GetAssetParams {
        api_key,
        operation_id: id,
    };

    let mut backoff = Duration::from_millis(100);
    loop {
        match get_asset(&get_params).await {
            Ok(asset_operation) if asset_operation.done.unwrap_or(false) => {
                if let Some(response) = asset_operation.response {
                    let id_str = response.asset_id;
                    let id = id_str.parse::<u64>().context("Asset ID must be a u64")?;

                    return match asset_type {
                        AssetType::DecalPng
                        | AssetType::DecalJpeg
                        | AssetType::DecalBmp
                        | AssetType::DecalTga => get_image_id(id).await,
                        _ => Ok(id),
                    };
                }
            }
            Ok(_) => {
                debug!("Asset operation not done, retrying...");
            }
            Err(Error::HttpStatusError { code: 404, .. }) => {
                debug!("Asset not found, retrying...");
            }
            Err(Error::HttpStatusError { code: 429, .. }) => {
                warn!("Rate limited, retrying...");
            }
            Err(e) => bail!("Failed to GET asset: {:?}", e),
        }

        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(10));
    }
}

const ANIMATION_URL: &str = "https://www.roblox.com/ide/publish/uploadnewanimation";

pub struct AnimationResult {
    pub asset_id: u64,
    pub csrf: String,
}

pub async fn get_csrf_token(cookie: String) -> anyhow::Result<String> {
    let client = Client::new();

    let response = client
        .post(ANIMATION_URL)
        .header("Cookie", cookie)
        .header("Content-Type", "application/xml")
        .header("Requester", "Client")
        .send()
        .await
        .context("Failed to get CSRF token")?;

    let csrf = response
        .headers()
        .get("x-csrf-token")
        .context("Failed to get CSRF token header")?
        .to_str()
        .context("Failed to convert CSRF token header to string")?;

    Ok(csrf.to_string())
}

pub async fn upload_animation(
    contents: Vec<u8>,
    display_name: String,
    cookie: String,
    csrf: Option<String>,
    creator: AssetCreator,
) -> anyhow::Result<AnimationResult> {
    let client = Client::new();

    let csrf = if let Some(token) = csrf {
        token
    } else {
        get_csrf_token(cookie.clone()).await?
    };

    let creator = match creator {
        AssetCreator::User(c) => ("userId", c.user_id.to_string()),
        AssetCreator::Group(c) => ("groupId", c.group_id.to_string()),
    };

    let response = client
        .post(ANIMATION_URL)
        .header("Cookie", cookie)
        .header("x-csrf-token", &csrf)
        .header("Content-Type", "application/xml")
        .header(
            "User-Agent",
            "RobloxStudio/WinInet RobloxApp/0.483.1.425021 (GlobalDist; RobloxDirectDownload)",
        )
        .header("Requester", "Client")
        .query(&[
            ("name", display_name),
            ("description", ASSET_DESCRIPTION.to_string()),
            ("isGamesAsset", "false".to_string()),
            creator,
            ("ispublic", "false".to_string()),
            ("assetTypeName", "animation".to_string()),
            ("AllID", "1".to_string()),
            ("allowComments", "false".to_string()),
        ])
        .body(contents)
        .send()
        .await
        .context("Failed to upload animation")?;

    let body = response
        .text()
        .await
        .context("Failed to parse request body to text")?;

    let id = body
        .parse::<u64>()
        .context("Failed to parse animation ID")?;

    Ok(AnimationResult { asset_id: id, csrf })
}
