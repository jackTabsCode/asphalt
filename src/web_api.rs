use crate::{
    asset::{Asset, AssetType},
    config,
};
use anyhow::{Context, bail};
use log::{debug, warn};
use reqwest::{RequestBuilder, Response, StatusCode, multipart};
use serde::{Deserialize, Serialize};
use std::{
    env,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::sync::Mutex;
use tokio::time::Instant;

const RATELIMIT_RESET_HEADER: &str = "x-ratelimit-reset";

const UPLOAD_URL: &str = "https://apis.roblox.com/assets/v1/assets";
const OPERATION_URL: &str = "https://apis.roblox.com/assets/v1/operations";
const ASSET_DESCRIPTION: &str = "Uploaded by Asphalt";
const MAX_DISPLAY_NAME_LENGTH: usize = 50;

pub struct WebApiClient {
    inner: reqwest::Client,
    api_key: String,
    creator: config::Creator,
    expected_price: Option<u32>,
    fatally_failed: AtomicBool,
    /// Shared rate limit state: when we can next make a request
    rate_limit_reset: Mutex<Option<Instant>>,
}

impl WebApiClient {
    pub fn new(api_key: String, creator: config::Creator, expected_price: Option<u32>) -> Self {
        WebApiClient {
            inner: reqwest::Client::new(),
            api_key,
            creator,
            expected_price,
            fatally_failed: AtomicBool::new(false),
            rate_limit_reset: Mutex::new(None),
        }
    }

    pub async fn upload(&self, asset: &Asset) -> anyhow::Result<u64> {
        if env::var("ASPHALT_TEST").is_ok() {
            return Ok(1337);
        }

        let file_name = asset.path.file_name().unwrap();
        let display_name = trim_display_name(file_name);

        let req = Request {
            display_name,
            asset_type: asset.ty,
            creation_context: CreationContext {
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
            .send_with_retry(|client| {
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

                client
                    .post(UPLOAD_URL)
                    .header("x-api-key", &self.api_key)
                    .multipart(form)
            })
            .await?;

        let body = res.text().await?;

        let operation: Operation = serde_json::from_str(&body)?;

        let id = self
            .poll_operation(operation.operation_id, &self.api_key)
            .await
            .context("Failed to poll operation")?;

        Ok(id)
    }

    async fn poll_operation(&self, id: String, api_key: &str) -> anyhow::Result<u64> {
        let mut delay = Duration::from_secs(1);
        const MAX_POLLS: u32 = 10;

        for attempt in 0..MAX_POLLS {
            let res = self
                .send_with_retry(|client| {
                    client
                        .get(format!("{OPERATION_URL}/{id}"))
                        .header("x-api-key", api_key)
                })
                .await?;

            let text = res.text().await?;

            let operation: Operation = serde_json::from_str(&text)?;

            if operation.done {
                if let Some(response) = operation.response {
                    return Ok(response.asset_id.parse()?);
                } else {
                    bail!("Operation completed but no response provided");
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
        F: Fn(&reqwest::Client) -> RequestBuilder,
    {
        if self.fatally_failed.load(Ordering::SeqCst) {
            bail!("A previous request failed due to a fatal error");
        }

        const MAX: u8 = 5;
        let mut attempt = 0;

        loop {
            {
                let reset = self.rate_limit_reset.lock().await;
                if let Some(reset_at) = *reset {
                    let now = Instant::now();
                    if reset_at > now {
                        let wait = reset_at - now;
                        drop(reset);
                        debug!("Waiting {:.2}ms for rate limit reset", wait.as_secs_f64());
                        tokio::time::sleep(wait).await;
                    }
                }
            }

            let res = make_req(&self.inner).send().await?;
            let status = res.status();

            match status {
                StatusCode::TOO_MANY_REQUESTS if attempt < MAX => {
                    let wait = res
                        .headers()
                        .get(RATELIMIT_RESET_HEADER)
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(Duration::from_secs)
                        .unwrap_or_else(|| Duration::from_secs(1 << attempt));

                    let reset_at = Instant::now() + wait;
                    {
                        let mut reset = self.rate_limit_reset.lock().await;
                        *reset = Some(reset_at);
                    }

                    warn!(
                        "Rate limited, retrying in {:.2} seconds",
                        wait.as_secs_f64()
                    );

                    tokio::time::sleep(wait).await;
                    attempt += 1;

                    continue;
                }
                StatusCode::OK => return Ok(res),
                _ => {
                    let body = res.text().await?;
                    self.fatally_failed.store(true, Ordering::SeqCst);
                    bail!("Request failed with status {status}:\n{body}");
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
