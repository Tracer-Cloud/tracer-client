pub mod types;

use crate::constants::TRACER_ANALYTICS_ENDPOINT;
use crate::utils::analytics::types::{AnalyticsEventType, AnalyticsPayload};
use reqwest::Client;
use std::collections::HashMap;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;

pub fn spawn_event(
    explicit_user_id: Option<String>,
    event: AnalyticsEventType,
    metadata: Option<HashMap<String, String>>,
) {
    tokio::spawn(send_event(explicit_user_id, event, metadata));
}
// COPIED: tracer-installer/src/installer/install.rs
pub async fn send_event(
    user_id: Option<String>,
    event: AnalyticsEventType,
    metadata: Option<HashMap<String, String>>,
) -> anyhow::Result<()> {
    let client = Client::new();
    let user_id = match user_id {
        Some(id) => id,
        None => match std::env::var("TRACER_USER_ID") {
            Ok(val) if !val.trim().is_empty() => val,
            _ => return Ok(()), // silently skip if no user ID
        },
    };

    let retry_strategy = ExponentialBackoff::from_millis(500).map(jitter).take(3);

    let payload = AnalyticsPayload {
        user_id: user_id.as_str(),
        event_name: event.as_str(),
        metadata,
    };
    Retry::spawn(retry_strategy, || async {
        let res = client
            .post(TRACER_ANALYTICS_ENDPOINT)
            .json(&payload)
            .send()
            .await?;

        if res.status().is_success() {
            Ok(())
        } else {
            eprintln!(
                "⚠️  Failed to send analytics event: {} [{}]",
                event.as_str(),
                res.status()
            );

            Err(anyhow::anyhow!("status = {}", res.status()))
        }
    })
    .await
}
