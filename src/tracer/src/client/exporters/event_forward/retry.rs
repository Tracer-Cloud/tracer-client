use super::telemetry;
use crate::process_identification::types::extracts::db::EventInsert;
use anyhow::Result;
use reqwest::Client;
use serde::Serialize;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

const MAX_RETRIES: usize = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);

#[derive(Serialize)]
struct EventPayload {
    events: Vec<EventInsert>,
}

/// Send events with retry logic
pub async fn send_events_with_retry(
    client: &Client,
    endpoint: &str,
    events: Vec<EventInsert>,
) -> Result<()> {
    if events.is_empty() {
        debug!("No events to send, skipping network call");
        return Ok(());
    }

    let payload = EventPayload { events };

    info!(
        "Sending {} events to {}",
        payload.events.len(),
        endpoint
    );

    for attempt in 1..=MAX_RETRIES {
        let start_time = Instant::now();

        match send_request(client, endpoint, &payload).await {
            Ok(()) => {
                info!(
                    "Successfully sent {} events on attempt {}, elapsed: {:?}",
                    payload.events.len(),
                    attempt,
                    start_time.elapsed()
                );
                return Ok(());
            }
            Err(e) => {
                let elapsed = start_time.elapsed();

                if should_retry(&e) && attempt < MAX_RETRIES {
                    warn!(
                        "Attempt {} failed (retrying): {}, elapsed: {:?}",
                        attempt, e, elapsed
                    );
                    tokio::time::sleep(RETRY_DELAY).await;
                } else {
                    error!(
                        "Attempt {} failed: {}, elapsed: {:?}",
                        attempt, e, elapsed
                    );

                    // Report final failure to Sentry
                    telemetry::report_network_failure_to_sentry(
                        endpoint,
                        &e,
                        payload.events.len(),
                        MAX_RETRIES,
                    );
                    return Err(e);
                }
            }
        }
    }

    unreachable!("Loop should always return")
}

/// Send a single HTTP request
async fn send_request(client: &Client, endpoint: &str, payload: &EventPayload) -> Result<()> {
    let response = client.post(endpoint).json(payload).send().await?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(anyhow::anyhow!("Server error {}: {}", status, body))
    }
}

/// Check if an error should trigger a retry
fn should_retry(error: &anyhow::Error) -> bool {
    // Retry on network errors or 5XX server errors
    if error.downcast_ref::<reqwest::Error>().is_some() {
        return true; // Always retry network errors
    }

    // Check if it's a server error (5XX)
    let error_str = error.to_string();
    if error_str.contains("Server error 5") {
        return true;
    }

    false
}
