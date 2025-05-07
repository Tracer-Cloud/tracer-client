use reqwest::Client;
use serde::Serialize;

#[derive(Serialize, Clone)]
struct EventPayload {
    run_name: String,
    run_id: String,
    pipeline_name: String,
    events: Vec<Event>,
}

pub struct LogForward {
    endpoint: String,
}

impl LogForward {
    pub async fn try_new() -> Result<Self> {
        self.endpoint = "http://sandbox.tracer.cloud/events".to_string(); //TODO get from confing file
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

        let client = Client::new();
        let res = client
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