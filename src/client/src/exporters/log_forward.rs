use serde::Serialize;
use tracing::debug;
use tracer_common::types::event::Event;
use crate::exporters::log_writer::LogWriter;
use reqwest::Client;
use anyhow::Result; // Import anyhow's Result type

#[derive(Serialize, Clone)]
struct EventPayload {
    run_name: String,
    run_id: String,
    pipeline_name: String,
    events: Vec<Event>,
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

        let events: Vec<Event> = data.into_iter().cloned().collect();

        if events.is_empty() {
            debug!("No data to send");
            return Ok(());
        }

        let payload = EventPayload {
            run_name: run_name.to_string(),
            run_id: run_id.to_string(),
            pipeline_name: pipeline_name.to_string(),
            events,
        };

        let res = self.client
            .post(&self.endpoint)
            .json(&payload)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow::anyhow!("Failed to send logs: {}", res.status()));
        }

        debug!(
            "Successfully sent {} events with run_name: {}, elapsed: {:?}",
            payload.events.len(),
            run_name,
            now.elapsed()
        );

        Ok(())
    }
}