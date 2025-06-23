use crate::constants::TRACER_ANALYTICS_ENDPOINT;
use crate::process_identification::types::analytics::{AnalyticsEventType, AnalyticsPayload};
use reqwest::Client;
use std::collections::HashMap;

pub async fn emit_analytic_event(
    explicit_user_id: Option<String>,
    event: AnalyticsEventType,
    metadata: Option<HashMap<String, String>>,
) -> anyhow::Result<()> {
    let user_id = match explicit_user_id {
        Some(id) => id,
        None => match std::env::var("TRACER_USER_ID") {
            Ok(val) if !val.trim().is_empty() => val,
            _ => return Ok(()), // silently skip if no user ID
        },
    };

    let payload = AnalyticsPayload {
        user_id: &user_id,
        event_name: event.as_str(),
        metadata,
    };

    let client = Client::new();
    let res = client
        .post(TRACER_ANALYTICS_ENDPOINT)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        tracing::error!(
            "Failed to send analytics event {:?} (status: {})",
            event,
            res.status()
        );
    }

    Ok(())
}
