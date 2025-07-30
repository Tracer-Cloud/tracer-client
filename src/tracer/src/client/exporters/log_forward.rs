use crate::client::exporters::log_writer::LogWriter;
use crate::process_identification::types::event::Event;
use crate::process_identification::types::extracts::db::EventInsert;
use anyhow::Result;
use log::error;
use reqwest::Client;
use serde::Serialize;
use std::convert::TryFrom;
use tracing::{debug, info};

#[derive(Serialize, Clone, Debug)]
struct EventPayload {
    events: Vec<EventInsert>,
}

pub struct LogForward {
    endpoint: String,
    client: Client,
}

impl LogForward {
    pub async fn try_new(log_forward_endpoint: &str) -> Result<Self> {
        Ok(LogForward {
            endpoint: log_forward_endpoint.to_string(),
            client: Client::new(),
        })
    }

    // Add a close method to match the expected interface
    pub async fn close(&self) -> Result<()> {
        // Client doesn't need explicit closing, but we provide
        // this method to satisfy the interface
        Ok(())
    }
}

impl LogWriter for LogForward {
    async fn batch_insert_events(
        &self,
        run_name: &str,
        run_id: &str,
        pipeline_name: &str,
        data: impl IntoIterator<Item = &Event>,
    ) -> Result<()> {
        let now = std::time::Instant::now();

        debug!(
            "run_id: {:?}, run_name: {:?}, pipeline_name: {:?}",
            run_id, run_name, pipeline_name
        );

        let events: Result<Vec<EventInsert>> = data
            .into_iter()
            .map(|event| EventInsert::try_from(event.clone()))
            .collect();

        let events = events?;

        if events.is_empty() {
            debug!("No data to send");
            return Ok(());
        }

        let payload = EventPayload { events };

        let payload_string = serde_json::to_string_pretty(&payload)
            .unwrap_or_else(|_| "Failed to serialize payload".to_string());

        debug!(
            "Sending payload to endpoint {} with {} events\nPayload: {}",
            self.endpoint,
            payload.events.len(),
            payload_string
        );

        match self.client.post(&self.endpoint).json(&payload).send().await {
            Ok(response) => {
                if response.status() == 200 {
                    println!(
                        "Successfully sent {} events with run_name: {}, elapsed: {:?}",
                        payload.events.len(),
                        run_name,
                        now.elapsed()
                    );
                    Ok(())
                } else {
                    let status = response.status();
                    error!(
                        "Failed to send events: {} [{}], Payload: {}",
                        run_name, status, payload_string
                    );
                    Err(anyhow::anyhow!(
                        "Failed to send events: {}, Payload: {}",
                        status,
                        payload_string
                    ))
                }
            }
            Err(e) => Err(anyhow::anyhow!("HTTP request failed: {:?}", e)),
        }
    }
}
