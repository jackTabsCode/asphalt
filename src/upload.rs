use anyhow::{Context, bail};
use log::{debug, warn};
use rbxcloud::rbx::{
    self,
    v1::assets::{
        AssetCreation, AssetCreationContext, CreateAssetParamsWithContents,
        GetAssetOperationParams, create_asset_with_contents, get_operation,
    },
};
use serde::Deserialize;
use std::time::Duration;

use crate::{
    asset::{Asset, AssetKind},
    config::{Creator, CreatorType},
};

const ASSET_DESCRIPTION: &str = "Uploaded by Asphalt";
const MAX_DISPLAY_NAME_LENGTH: usize = 50;

pub async fn upload_cloud(
    client: reqwest::Client,
    asset: &Asset,
    cookie: String,
    csrf: Option<String>,
    api_key: String,
    creator: &Creator,
) -> anyhow::Result<u64> {
    let params = CreateAssetParamsWithContents {
        contents: &asset.data,
        api_key: api_key.clone(),
        asset: AssetCreation {
            asset_type: asset.kind.clone().try_into()?,
            display_name: trim_display_name(&asset.path.to_string_lossy()),
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
        api_key: api_key.clone(),
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
                        AssetKind::Decal(_) => get_image_id(client, cookie, csrf, id).await,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Roblox {
    data: Vec<Data>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    asset: CreatorAsset,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatorAsset {
    texture_id: Option<u64>,
}

async fn get_image_id(
    client: reqwest::Client,
    cookie: String,
    csrf: Option<String>,
    asset_id: u64,
) -> anyhow::Result<u64> {
    let url = "https://apis.roblox.com/toolbox-service/v1/items/details";

    let csrf = if let Some(token) = csrf {
        token
    } else {
        get_csrf_token(&client, cookie.clone()).await?
    };

    let response = client
        .get(url)
        .header("Cookie", cookie)
        .header("x-csrf-token", &csrf)
        .header(
            "User-Agent",
            "RobloxStudio/WinInet RobloxApp/0.483.1.425021 (GlobalDist; RobloxDirectDownload)",
        )
        .header("Requester", "Client")
        .query(&[("assetIds", asset_id.to_string())])
        .send()
        .await
        .context("Failed to get image ID")?;

    let body = response.text().await?;

    let roblox: Roblox = serde_json::from_str(&body).context("Failed to parse response body")?;

    let image_id = roblox
        .data
        .get(0)
        .and_then(|d| d.asset.texture_id)
        .context("Failed to get texture ID")?;

    Ok(image_id)
}

pub struct AnimationResult {
    pub asset_id: u64,
    pub csrf: String,
}

const ANIMATION_URL: &str = "https://www.roblox.com/ide/publish/uploadnewanimation";

pub async fn upload_animation(
    client: reqwest::Client,
    asset: &Asset,
    cookie: String,
    csrf: Option<String>,
    creator: &Creator,
) -> anyhow::Result<AnimationResult> {
    let display_name = asset.path.to_string_lossy().to_string();

    let csrf = if let Some(token) = csrf {
        token
    } else {
        get_csrf_token(&client, cookie.clone()).await?
    };

    let creator_ty = match creator.ty {
        CreatorType::User => "userId",
        CreatorType::Group => "groupId",
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
            ("name", trim_display_name(&display_name)),
            ("description", ASSET_DESCRIPTION.to_string()),
            ("isGamesAsset", "false".to_string()),
            (creator_ty, creator.id.to_string()),
            ("ispublic", "false".to_string()),
            ("assetTypeName", "animation".to_string()),
            ("AllID", "1".to_string()),
            ("allowComments", "false".to_string()),
        ])
        .body(asset.data.clone())
        .send()
        .await
        .context("Failed to send animation upload request")?
        .error_for_status()
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

pub async fn get_csrf_token(client: &reqwest::Client, cookie: String) -> anyhow::Result<String> {
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

fn trim_display_name(name: &str) -> String {
    let full_path = name.to_string();
    if full_path.len() > MAX_DISPLAY_NAME_LENGTH {
        let start_index = full_path.len().saturating_sub(MAX_DISPLAY_NAME_LENGTH);
        full_path[start_index..].to_string()
    } else {
        full_path
    }
}