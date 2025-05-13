use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

pub mod upload;

pub async fn send_http_get(
    url: &str,
    api_key: Option<&str>,
    timeout_duration: Option<Duration>,
) -> Result<(u16, String)> {
    let client = Client::new();
    let mut response = client.get(url);

    if let Some(api_key) = api_key {
        response = response.header("x-api-key", api_key)
    }

    if let Some(timeout_duration) = timeout_duration {
        response = response.timeout(timeout_duration)
    }

    let response = response.send().await.context("Failed to send http get")?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Unknown error".to_string());

    Ok((status.as_u16(), response_text))
}

pub async fn send_http_body(
    url: &str,
    api_key: &str,
    request_body: &Value,
) -> Result<(u16, String)> {
    let client = Client::new();
    let response = client
        .post(url)
        .header("x-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(request_body)
        .send()
        .await
        .context("Failed to send event data")?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Unknown error".to_string());

    Ok((status.as_u16(), response_text))
}
