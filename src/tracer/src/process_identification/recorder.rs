use crate::daemon::structs::PipelineMetadata;
use crate::process_identification::types::current_run::RunMetadata;
use crate::process_identification::types::event::attributes::EventAttributes;
use crate::process_identification::types::event::{Event, ProcessStatus};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct EventDispatcher {
    pipeline: Arc<Mutex<PipelineMetadata>>,
    run: RunMetadata,
    tx: Sender<Event>,
}

impl EventDispatcher {
    pub fn new(pipeline: Arc<Mutex<PipelineMetadata>>, run: RunMetadata, tx: Sender<Event>) -> Self {
        EventDispatcher { pipeline, run, tx }
    }

    pub async fn log_with_metadata(
        &self,
        process_status: ProcessStatus,
        body: String,
        attributes: Option<EventAttributes>,
        timestamp: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        let pipeline = &self.pipeline.lock().await;
        let run = &self.run;
        let event = Event::builder()
            .body(body)
            .timestamp(timestamp.unwrap_or_else(Utc::now))
            .process_status(process_status)
            .pipeline_name(Some(pipeline.name.clone()))
            .run_name(Some(run.name.clone()))
            .run_id(Some(run.id.clone()))
            .tags(Some(pipeline.tags.clone()))
            .attributes(attributes)
            .trace_id(run.trace_id.clone())
            .build();

        self.tx.send(event).await?;
        Ok(())
    }

    pub async fn log(
        &self,
        process_status: ProcessStatus,
        message: String,
        attributes: Option<EventAttributes>,
        timestamp: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        self.log_with_metadata(process_status, message, attributes, timestamp)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_identification::types::event::attributes::{
        process::DataSetsProcessed, EventAttributes,
    };
    use chrono::TimeZone;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_event_with_metadata() {
        let (pipeline, run) = create_test_pipeline();
        let (tx, mut rx) = mpsc::channel(10);

        let recorder = EventDispatcher::new(pipeline, run, tx);

        let message = "Test log message".to_string();
        let fixed_time = Utc.with_ymd_and_hms(2025, 4, 30, 12, 0, 0).unwrap();

        recorder
            .log_with_metadata(
                ProcessStatus::ToolExecution,
                message.clone(),
                None,
                Some(fixed_time),
            )
            .await
            .unwrap();

        // Verify the event was sent correctly
        let event = rx.recv().await.unwrap();
        assert_eq!(event.body, message);
        assert_eq!(event.timestamp, fixed_time);
        assert_eq!(event.process_status, ProcessStatus::ToolExecution);
        assert_eq!(event.pipeline_name, Some("test_pipeline".to_string()));
        assert_eq!(event.run_name, Some("test_run".to_string()));
        assert_eq!(event.run_id, Some("test-id-123".to_string()));
    }

    #[tokio::test]
    async fn test_log_method() {
        let (pipeline, run) = create_test_pipeline();
        let (tx, mut rx) = mpsc::channel(10);

        let recorder = EventDispatcher::new(pipeline, run, tx);

        let message = "Test log via standard method".to_string();

        // Create test attributes
        let attributes = Some(EventAttributes::ProcessDatasetStats(DataSetsProcessed {
            datasets: "dataset1,dataset2".to_string(),
            total: 2,
            trace_id: Some(Uuid::new_v4().to_string()),
        }));

        // Call the log method
        recorder
            .log(
                ProcessStatus::MetricEvent,
                message.clone(),
                attributes.clone(),
                None,
            )
            .await
            .unwrap();

        // Verify the event was sent correctly
        let event = rx.recv().await.unwrap();
        assert_eq!(event.body, message);
        assert_eq!(event.process_status, ProcessStatus::MetricEvent);
        assert_eq!(event.pipeline_name, Some("test_pipeline".to_string()));
        assert_eq!(event.run_name, Some("test_run".to_string()));
        assert_eq!(event.run_id, Some("test-id-123".to_string()));

        // Check that attributes were passed correctly
        match &event.attributes {
            Some(EventAttributes::ProcessDatasetStats(stats)) => {
                assert_eq!(stats.datasets, "dataset1,dataset2");
                assert_eq!(stats.total, 2);
                assert!(stats.trace_id.is_some());
            }
            _ => panic!("Expected ProcessDatasetStats attributes"),
        }
    }

    #[tokio::test]
    async fn test_log_handles_channel_errors() {
        // Create a channel with capacity 1
        let (pipeline, run) = create_test_pipeline();
        let (tx, _rx) = mpsc::channel::<Event>(1);
        let recorder = EventDispatcher::new(pipeline, run, tx.clone());

        // Close the receiver to force send errors
        drop(_rx);

        // Attempt to log - this should result in an error
        let result = recorder
            .log(
                ProcessStatus::Alert,
                "This message should fail to send".to_string(),
                None,
                None,
            )
            .await;

        // Verify the error occurred
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_log_with_trace_id_from_run() {
        let trace_id = "trace-id-xyz".to_string();

        let (pipeline, run) = create_test_pipeline();

        let (tx, mut rx) = mpsc::channel(10);
        let recorder = EventDispatcher::new(pipeline, run, tx);

        let message = "Logging with trace_id".to_string();

        recorder
            .log(ProcessStatus::ToolExecution, message.clone(), None, None)
            .await
            .unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.body, message);
        assert_eq!(event.trace_id, Some(trace_id));
    }

    // Helper function to create a test pipeline
    fn create_test_pipeline() -> (Arc<Mutex<PipelineMetadata>>, RunMetadata) {
        let trace_id = "trace-id-xyz".to_string();
        // Build a custom run with trace_id
        let run = RunMetadata {
            name: "test_run".to_string(),
            id: "test-id-123".to_string(),
            start_time: Utc::now(),
            cost_summary: None,
            trace_id: Some(trace_id.clone()),
        };

        let pipeline = Arc::new(Mutex::new(PipelineMetadata {
            name: "test_pipeline".to_string(),
            run_snapshot: None,
            tags: Default::default(),
            is_dev: true,
            start_time: Default::default(),
        }));
        (pipeline, run)
    }
}
