use crate::{
    asset::{Asset, AssetType},
    auth::Auth,
    config::{Creator, CreatorType},
};
use anyhow::{Context, bail};
use log::{debug, warn};
use reqwest::{
    RequestBuilder, Response, StatusCode,
    header::{self},
    multipart,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const UPLOAD_URL: &str = "https://apis.roblox.com/assets/v1/assets";
const OPERATION_URL: &str = "https://apis.roblox.com/assets/v1/operations";
const ASSET_DESCRIPTION: &str = "Uploaded by Asphalt";
const MAX_DISPLAY_NAME_LENGTH: usize = 50;

pub struct WebApiClient {
    inner: reqwest::Client,
    auth: Auth,
    creator: Creator,
    expected_price: Option<u32>,
}

impl WebApiClient {
    pub fn new(auth: Auth, creator: Creator, expected_price: Option<u32>) -> Self {
        WebApiClient {
            inner: reqwest::Client::new(),
            auth,
            creator,
            expected_price,
        }
    }

    pub async fn upload(&self, asset: &Asset) -> anyhow::Result<u64> {
        let api_key = self
            .auth
            .api_key
            .clone()
            .context("An API key is necessary to upload")?;

        let file_name = asset.path.file_name().unwrap();
        let display_name = trim_display_name(file_name);

        let req = WebAssetRequest {
            display_name,
            asset_type: asset.ty.clone(),
            creation_context: WebAssetRequestCreationContext {
                creator: self.creator.clone().into(),
                expected_price: self.expected_price,
            },
            description: ASSET_DESCRIPTION,
        };

        let len = asset.data.len() as u64;
        let req_json = serde_json::to_string(&req)?;
        let mime = req.asset_type.file_type().to_owned();
        let name = file_name.to_owned();

        let res = self
            .send_with_retry(|| {
                let file_part = multipart::Part::stream_with_length(
                    reqwest::Body::from(asset.data.clone()),
                    len,
                )
                .file_name(name.clone())
                .mime_str(&mime)
                .unwrap();

                let form = multipart::Form::new()
                    .text("request", req_json.clone())
                    .part("fileContent", file_part);

                self.inner
                    .post(UPLOAD_URL)
                    .header("x-api-key", &api_key)
                    .multipart(form)
            })
            .await?;

        let status = res.status();
        let body = res.text().await?;

        if status.is_success() {
            let operation: WebAssetOperation = serde_json::from_str(&body)?;

            match self.poll_operation(operation.operation_id, &api_key).await {
                Ok(Some(id)) => Ok(id),
                Ok(None) => bail!("Failed to get asset ID"),
                Err(e) => Err(e),
            }
        } else {
            bail!("Failed to upload asset: {} - {}", status, body)
        }
    }

    async fn poll_operation(&self, id: String, api_key: &str) -> anyhow::Result<Option<u64>> {
        let mut delay = Duration::from_secs(1);
        const MAX_POLLS: u32 = 10;

        for attempt in 0..MAX_POLLS {
            let res = self
                .send_with_retry(|| {
                    self.inner
                        .get(format!("{OPERATION_URL}/{id}"))
                        .header("x-api-key", api_key)
                })
                .await?;

            let status = res.status();
            let text = res.text().await?;

            if !status.is_success() {
                bail!("Failed to poll operation: {} - {}", status, text);
            }

            let operation: WebAssetOperation = serde_json::from_str(&text)?;

            if operation.done {
                if let Some(response) = operation.response {
                    return Ok(Some(response.asset_id.parse()?));
                } else {
                    bail!("Operation completed but no response provided")
                }
            }

            debug!("Operation not done yet");

            if attempt < MAX_POLLS - 1 {
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
        }

        bail!("Operation polling exceeded maximum retries")
    }

    async fn send_with_retry<F>(&self, make_req: F) -> anyhow::Result<Response>
    where
        F: Fn() -> RequestBuilder,
    {
        const MAX: u8 = 5;
        let mut attempt = 0;

        loop {
            let res = make_req().send().await?;

            match res.status() {
                StatusCode::TOO_MANY_REQUESTS if attempt < MAX => {
                    let wait = res
                        .headers()
                        .get(header::RETRY_AFTER)
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(Duration::from_secs)
                        .unwrap_or_else(|| Duration::from_secs(1 << attempt));

                    tokio::time::sleep(wait).await;
                    attempt += 1;

                    warn!(
                        "Rate limited, retrying in {} seconds",
                        wait.as_millis() / 1000
                    );

                    continue;
                }
                _ => return Ok(res),
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebAssetRequest {
    asset_type: AssetType,
    display_name: String,
    description: &'static str,
    creation_context: WebAssetRequestCreationContext,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebAssetRequestCreationContext {
    creator: WebAssetCreator,
    expected_price: Option<u32>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum WebAssetCreator {
    User(WebAssetUserCreator),
    Group(WebAssetGroupCreator),
}

impl From<Creator> for WebAssetCreator {
    fn from(value: Creator) -> Self {
        match value.ty {
            CreatorType::User => WebAssetCreator::User(WebAssetUserCreator {
                user_id: value.id.to_string(),
            }),
            CreatorType::Group => WebAssetCreator::Group(WebAssetGroupCreator {
                group_id: value.id.to_string(),
            }),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebAssetUserCreator {
    user_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebAssetGroupCreator {
    group_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebAssetOperation {
    done: bool,
    operation_id: String,
    response: Option<WebAssetOperationResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebAssetOperationResponse {
    asset_id: String,
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
