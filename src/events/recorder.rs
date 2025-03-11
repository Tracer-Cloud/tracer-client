use crate::types::event::{attributes::EventAttributes, Event};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events recorder for each pipeline run
pub struct EventRecorder {
    events: Vec<Event>,
    run_name: Option<String>,
    run_id: Option<String>,
    // NOTE: Tying a pipeline_name to the events recorder because, you can only start one pipeline at a time
    pipeline_name: Option<String>,
    aws_batch_job_id: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum EventType {
    NewRun,
    FinishedRun,
    ToolExecution,
    FinishedToolExecution,
    ToolMetricEvent,
    MetricEvent,
    SyslogEvent,
    RunStatusMessage,
    Alert,
    DataSamplesEvent,
    TestEvent, // Added TestEvent variant
    NextflowLogEvent,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::NewRun => "new_run",
            EventType::FinishedRun => "finished_run",
            EventType::ToolExecution => "tool_execution",
            EventType::FinishedToolExecution => "finished_tool_execution",
            EventType::MetricEvent => "metric_event",
            EventType::SyslogEvent => "syslog_event",
            EventType::ToolMetricEvent => "tool_metric_event",
            EventType::TestEvent => "test_event", // Handle TestEvent
            EventType::RunStatusMessage => "run_status_message",
            EventType::Alert => "alert",
            EventType::DataSamplesEvent => "datasets_in_process",
            EventType::NextflowLogEvent => "nextflow_log_event",
        }
    }
}

impl EventRecorder {
    pub fn new(
        pipeline_name: Option<String>,
        run_name: Option<String>,
        run_id: Option<String>,
    ) -> Self {
        // Try to get AWS_BATCH_JOB_ID from environment
        let aws_batch_job_id = std::env::var("AWS_BATCH_JOB_ID").ok();

        if let Some(job_id) = &aws_batch_job_id {
            println!("Found AWS Batch Job ID: {}", job_id);
            tracing::info!("Found AWS Batch Job ID: {}", job_id);
        } else {
            println!("No AWS Batch Job ID found in environment");
            tracing::debug!("No AWS Batch Job ID found in environment");
        }

        EventRecorder {
            events: Vec::new(),
            run_id,
            run_name,
            pipeline_name,
            aws_batch_job_id,
            tags: Vec::new(),
        }
    }

    pub(crate) fn update_run_details(
        &mut self,
        pipeline_name: Option<String>,
        run_name: Option<String>,
        run_id: Option<String>,
        tags: Vec<String>,
    ) {
        self.run_name = run_name;
        self.run_id = run_id;
        self.pipeline_name = pipeline_name;
        self.tags = tags;
        // Check for AWS_BATCH_JOB_ID in case it wasn't available during initialization
        let new_job_id = std::env::var("AWS_BATCH_JOB_ID").ok();
        if new_job_id != self.aws_batch_job_id {
            if let Some(job_id) = &new_job_id {
                tracing::info!("Updated AWS Batch Job ID: {}", job_id);
            }
            self.aws_batch_job_id = new_job_id;
        }
    }

    pub fn record_event(
        &mut self,
        event_type: EventType,
        message: String,
        attributes: Option<EventAttributes>,
        timestamp: Option<DateTime<Utc>>,
    ) {
        let event = Event {
            timestamp: timestamp.unwrap_or_else(Utc::now),
            message,
            event_type: "process_status".to_owned(),
            process_type: "pipeline".to_owned(),
            process_status: event_type.as_str().to_owned(),
            attributes,
            // NOTE: not a fan of constant cloning so would look for an alt
            run_name: self.run_name.clone(),
            run_id: self.run_id.clone(),
            pipeline_name: self.pipeline_name.clone(),
            aws_batch_job_id: self.aws_batch_job_id.clone(),
            tags: self.tags.clone(),
        };
        self.events.push(event);
    }

    pub fn get_events(&self) -> &[Event] {
        &self.events
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.events.len()
    }
}

impl Default for EventRecorder {
    fn default() -> Self {
        Self::new(None, None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_record_event() {
        let mut recorder = EventRecorder::default();
        let message = "[event_recorder.rs]Test event".to_string();
        let attributes = Some(EventAttributes::Other(json!({"key": "value"})));

        recorder.record_event(
            EventType::ToolExecution,
            message.clone(),
            attributes.clone(),
            None,
        );

        assert_eq!(recorder.len(), 1);

        let event = &recorder.get_events()[0];
        assert_eq!(event.message, message);
        assert_eq!(event.event_type, "process_status");
        assert_eq!(event.process_type, "pipeline");
        assert_eq!(event.process_status, "tool_execution");
        assert!(matches!(
            event.attributes.clone().unwrap(),
            EventAttributes::Other(_)
        ));
    }

    #[test]
    fn test_clear_events() {
        let mut recorder = EventRecorder::default();
        recorder.record_event(
            EventType::ToolExecution,
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
        assert_eq!(EventType::FinishedRun.as_str(), "finished_run");
        assert_eq!(EventType::ToolExecution.as_str(), "tool_execution");
        assert_eq!(EventType::MetricEvent.as_str(), "metric_event");
        assert_eq!(EventType::TestEvent.as_str(), "test_event");
    }

    #[test]
    fn test_record_test_event() {
        let mut recorder = EventRecorder::default();
        let message = "Test event for testing".to_string();
        let attributes = Some(EventAttributes::Other(json!({"test_key": "test_value"})));

        recorder.record_event(
            EventType::TestEvent,
            message.clone(),
            attributes.clone(),
            None,
        );

        assert_eq!(recorder.len(), 1);

        let event = &recorder.get_events()[0];
        assert_eq!(event.message, message);
        assert_eq!(event.event_type, "process_status");
        assert_eq!(event.process_type, "pipeline");
        assert_eq!(event.process_status, "test_event");
        assert!(matches!(
            event.attributes.clone().unwrap(),
            EventAttributes::Other(_)
        ));
    }
}
