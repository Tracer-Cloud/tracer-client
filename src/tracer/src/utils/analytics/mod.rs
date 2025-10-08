pub mod types;

use crate::constants::TRACER_ANALYTICS_ENDPOINT;
use crate::utils::analytics::types::{AnalyticsEventType, AnalyticsPayload};
use crate::utils::env::detect_environment_type;
use reqwest::Client;
use std::collections::HashMap;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;

pub fn spawn_event(user_id: String, event: AnalyticsEventType) {
    tokio::spawn(send_event(user_id, event));
}

pub async fn send_event(user_id: String, event: AnalyticsEventType) -> anyhow::Result<()> {
    let client = Client::new();
    let retry_strategy = ExponentialBackoff::from_millis(500).map(jitter).take(3);

    // Ensure the environment is set in metadata
    let mut metadata = HashMap::new();
    if !metadata.contains_key("environment") {
        metadata.insert("environment".to_string(), detect_environment_type());
    }

    let payload = AnalyticsPayload {
        user_id: user_id.as_str(),
        event_name: event.as_str(),
        metadata: Some(metadata),
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
                "Failed to send analytics event: {} [{}]",
                event.as_str(),
                res.status()
            );

            Err(anyhow::anyhow!("status = {}", res.status()))
        }
    })
    .await
}
