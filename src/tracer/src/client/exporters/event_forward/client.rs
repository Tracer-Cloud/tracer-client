use super::retry;
use crate::client::exporters::event_writer::EventWriter;
use crate::process_identification::types::event::Event;
use crate::process_identification::types::extracts::db::EventInsert;
use anyhow::Result;
use reqwest::Client;
use std::convert::TryFrom;
use std::time::Instant;
use tracing::debug;

/// Configuration for event forwarding
#[derive(Clone)]
pub struct EventForwardConfig {
    pub endpoint: String,
    pub client: Client,
}

/// Create a new event forward configuration
pub async fn create_event_forward_config(endpoint: &str) -> Result<EventForwardConfig> {
    Ok(EventForwardConfig {
        endpoint: endpoint.to_string(),
        client: Client::new(),
    })
}

/// Convert events to database format
fn convert_events_to_inserts<'a>(
    events: impl IntoIterator<Item = &'a Event>,
) -> Result<Vec<EventInsert>> {
    events
        .into_iter()
        .map(|event| EventInsert::try_from(event.clone()))
        .collect()
}

/// Send events with timing and logging
async fn send_events_with_timing(
    config: &EventForwardConfig,
    events: Vec<EventInsert>,
) -> Result<()> {
    let start_time = Instant::now();

    retry::send_events_with_retry(&config.client, &config.endpoint, events).await?;

    debug!("Event forwarding completed in {:?}", start_time.elapsed());
    Ok(())
}

/// Forward events to remote endpoint (pure functional interface)
pub async fn forward_events<'a>(
    config: &EventForwardConfig,
    events: impl IntoIterator<Item = &'a Event>,
) -> Result<()> {
    let inserts = convert_events_to_inserts(events)?;
    send_events_with_timing(config, inserts).await
}

/// HTTP client for forwarding events to a remote endpoint (compatibility wrapper)
pub struct EventForward {
    config: EventForwardConfig,
}

impl EventForward {
    /// Create a new EventForward client
    pub async fn try_new(event_forward_endpoint: &str) -> Result<Self> {
        Ok(EventForward {
            config: create_event_forward_config(event_forward_endpoint).await?,
        })
    }

    /// Close the client (no-op for HTTP client)
    pub async fn close(&self) -> Result<()> {
        Ok(())
    }
}

impl EventWriter for EventForward {
    async fn batch_insert_events(&self, data: impl IntoIterator<Item = &Event>) -> Result<()> {
        forward_events(&self.config, data).await
    }
}
