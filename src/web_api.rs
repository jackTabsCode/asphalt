use crate::{
    asset::{Asset, AssetType},
    config,
};
use anyhow::{Context, anyhow};
use log::{debug, warn};
use reqwest::{
    RequestBuilder, Response, StatusCode,
    header::{self},
    multipart,
};
use serde::{Deserialize, Serialize};
use std::{env, time::Duration};

const UPLOAD_URL: &str = "https://apis.roblox.com/assets/v1/assets";
const OPERATION_URL: &str = "https://apis.roblox.com/assets/v1/operations";
const ASSET_DESCRIPTION: &str = "Uploaded by Asphalt";
const MAX_DISPLAY_NAME_LENGTH: usize = 50;

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("Fatal error: (status: {status}, message: {message}, body: {body})")]
    Fatal {
        status: StatusCode,
        message: String,
        body: String,
    },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct WebApiClient {
    inner: reqwest::Client,
    api_key: Option<String>,
    creator: config::Creator,
    expected_price: Option<u32>,
}

impl WebApiClient {
    pub fn new(
        api_key: Option<String>,
        creator: config::Creator,
        expected_price: Option<u32>,
    ) -> Self {
        WebApiClient {
            inner: reqwest::Client::new(),
            api_key,
            creator,
            expected_price,
        }
    }

    pub async fn upload(&self, asset: &Asset) -> Result<u64, UploadError> {
        if env::var("ASPHALT_TEST").is_ok() {
            return Ok(1337);
        }

        let api_key = self
            .api_key
            .clone()
            .context("An API key is necessary to upload")?;

        let file_name = asset.path.file_name().unwrap();
        let display_name = trim_display_name(file_name);

        let req = Request {
            display_name,
            asset_type: asset.ty.clone(),
            creation_context: CreationContext {
                creator: self.creator.clone().into(),
                expected_price: self.expected_price,
            },
            description: ASSET_DESCRIPTION,
        };

        let len = asset.data.len() as u64;
        let req_json = serde_json::to_string(&req).map_err(anyhow::Error::from)?;
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

        let body = res.text().await.map_err(anyhow::Error::from)?;

        let operation: Operation = serde_json::from_str(&body).map_err(anyhow::Error::from)?;

        let id = self
            .poll_operation(operation.operation_id, &api_key)
            .await?;

        Ok(id)
    }

    async fn poll_operation(&self, id: String, api_key: &str) -> Result<u64, UploadError> {
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
            let text = res.text().await.map_err(anyhow::Error::from)?;

            if !status.is_success() {
                return Err(anyhow!("Failed to poll operation: {} - {}", status, text).into());
            }

            let operation: Operation = serde_json::from_str(&text).map_err(anyhow::Error::from)?;

            if operation.done {
                if let Some(response) = operation.response {
                    return Ok(response.asset_id.parse().map_err(anyhow::Error::from)?);
                } else {
                    return Err(anyhow!("Operation completed but no response provided").into());
                }
            }

            debug!("Operation not done yet");

            if attempt < MAX_POLLS - 1 {
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
        }

        Err(anyhow!("Operation polling exceeded maximum retries").into())
    }

    async fn send_with_retry<F>(&self, make_req: F) -> Result<Response, UploadError>
    where
        F: Fn() -> RequestBuilder,
    {
        const MAX: u8 = 5;
        let mut attempt = 0;

        loop {
            let res = make_req().send().await.map_err(anyhow::Error::from)?;
            let status = res.status();

            match status {
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
                StatusCode::OK => return Ok(res),
                _ => {
                    let body = res.text().await.map_err(anyhow::Error::from)?;
                    let message = extract_error_message(&body);

                    return Err(UploadError::Fatal {
                        status,
                        message,
                        body,
                    });
                }
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    asset_type: AssetType,
    display_name: String,
    description: &'static str,
    creation_context: CreationContext,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreationContext {
    creator: Creator,
    expected_price: Option<u32>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Creator {
    User(UserCreator),
    Group(GroupCreator),
}

impl From<config::Creator> for Creator {
    fn from(value: config::Creator) -> Self {
        match value.ty {
            config::CreatorType::User => Creator::User(UserCreator {
                user_id: value.id.to_string(),
            }),
            config::CreatorType::Group => Creator::Group(GroupCreator {
                group_id: value.id.to_string(),
            }),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserCreator {
    user_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupCreator {
    group_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Operation {
    done: bool,
    operation_id: String,
    response: Option<OperationResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OperationResponse {
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

#[derive(Deserialize)]
struct ErrorBody {
    errors: Vec<ErrorItem>,
}

#[derive(Deserialize)]
struct ErrorItem {
    message: String,
}

fn extract_error_message(body: &str) -> String {
    let error_body: ErrorBody = serde_json::from_str(body).unwrap();
    error_body.errors[0].message.clone()
}
