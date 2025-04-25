// use crate::event::attributes::EventAttributes;
// use crate::event::{Event, EventType, ProcessStatus, ProcessType};
// use crate::pipeline_tags::PipelineTags;
// use chrono::{DateTime, Utc};
//
// /// Events recorder for each pipeline run

use crate::current_run::PipelineMetadata;
use crate::types::event::attributes::EventAttributes;
use crate::types::event::{Event, EventType, ProcessStatus, ProcessType};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct StructLogRecorder {
    pipeline: Arc<RwLock<PipelineMetadata>>,
    tx: Sender<Event>,
}

impl StructLogRecorder {
    pub fn new(pipeline: Arc<RwLock<PipelineMetadata>>, tx: Sender<Event>) -> Self {
        StructLogRecorder { pipeline, tx }
    }

    pub async fn log_with_metadata(
        &self,
        process_status: ProcessStatus,
        body: String,
        attributes: Option<EventAttributes>,
        timestamp: Option<DateTime<Utc>>,
        pipeline: &PipelineMetadata,
    ) -> anyhow::Result<()> {
        let event = Event::builder()
            .body(body)
            .timestamp(timestamp.unwrap_or_else(Utc::now))
            .process_status(process_status)
            .pipeline_name(Some(pipeline.pipeline_name.clone()))
            .run_name(pipeline.run.as_ref().map(|m| m.name.clone()))
            .run_id(pipeline.run.as_ref().map(|m| m.id.clone()))
            .tags(Some(pipeline.tags.clone()))
            .attributes(attributes)
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
        let run_metadata = self.pipeline.read().await;
        self.log_with_metadata(
            process_status,
            message,
            attributes,
            timestamp,
            &run_metadata,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::attributes::EventAttributes;
    use crate::event::ProcessStatus;
    use serde_json::json;

    #[test]
    fn test_record_event() {
        let mut recorder = EventRecorder::default();
        let message = "[event_recorder.rs]Test event".to_string();
        let attributes = Some(EventAttributes::Other(json!({"key": "value"})));

        recorder.record_event(
            ProcessStatus::ToolExecution,
            message.clone(),
            attributes.clone(),
            None,
        );

        assert_eq!(recorder.len(), 1);

        let event = &recorder.get_events()[0];
        assert_eq!(event.body, message);
        assert_eq!(event.event_type, EventType::ProcessStatus);
        assert_eq!(event.process_type, ProcessType::Pipeline);
        assert_eq!(event.process_status, ProcessStatus::ToolExecution);
        assert!(matches!(
            event.attributes.clone().unwrap(),
            EventAttributes::ProcessDatasetStats(_)
        ));
    }

    #[test]
    fn test_clear_events() {
        let mut recorder = EventRecorder::default();
        recorder.record_event(
            ProcessStatus::ToolExecution,
            "Test event".to_string(),
            None,
            None,
        );
        assert_eq!(recorder.len(), 1);

        recorder.clear();
        assert!(recorder.is_empty());
    }

    #[test]
    fn test_event_type_as_str() {
        assert_eq!(&ProcessStatus::FinishedRun.to_string(), "finished_run");
        assert_eq!(&ProcessStatus::ToolExecution.to_string(), "tool_execution");
        assert_eq!(&ProcessStatus::MetricEvent.to_string(), "metric_event");
        assert_eq!(&ProcessStatus::TestEvent.to_string(), "test_event");
    }

    #[test]
    fn test_record_test_event() {
        let mut recorder = EventRecorder::default();
        let message = "Test event for testing".to_string();
        let attributes = Some(EventAttributes::ProcessDatasetStats(DataSetsProcessed {
            datasets: "".to_string(),
            total: 2,
            trace_id: None,
        }));

        recorder.record_event(
            ProcessStatus::TestEvent,
            message.clone(),
            attributes.clone(),
            None,
        );

        assert_eq!(recorder.len(), 1);

        let event = &recorder.get_events()[0];
        assert_eq!(event.body, message);
        assert_eq!(event.event_type, EventType::ProcessStatus);
        assert_eq!(event.process_type, ProcessType::Pipeline);
        assert_eq!(event.process_status, ProcessStatus::TestEvent);
        assert!(matches!(
            event.attributes.clone().unwrap(),
            EventAttributes::ProcessDatasetStats(_)
        ));
    }
}
