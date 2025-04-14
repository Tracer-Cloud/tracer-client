use crate::event::attributes::EventAttributes;
use crate::event::event::{Event, EventType, ProcessStatus, ProcessType};
use crate::pipeline_tags::PipelineTags;
use chrono::{DateTime, Utc};

/// Events recorder for each pipeline run
/// todo: move to tracer_lib!
pub struct EventRecorder {
    events: Vec<Event>,
    run_name: Option<String>,
    run_id: Option<String>,
    // NOTE: Tying a pipeline_name to the events recorder because, you can only start one pipeline at a time
    pipeline_name: Option<String>,
    tags: Option<PipelineTags>,
}

impl EventRecorder {
    pub fn new(
        pipeline_name: Option<String>,
        run_name: Option<String>,
        run_id: Option<String>,
    ) -> Self {
        EventRecorder {
            events: Vec::new(),
            run_id,
            run_name,
            pipeline_name,
            tags: None,
        }
    }

    pub fn update_run_details(
        &mut self,
        pipeline_name: Option<String>,
        run_name: Option<String>,
        run_id: Option<String>,
        tags: Option<PipelineTags>,
    ) {
        self.run_name = run_name;
        self.run_id = run_id;
        self.pipeline_name = pipeline_name;
        self.tags = tags;
    }

    pub fn record_event(
        &mut self,
        process_status: ProcessStatus,
        message: String,
        attributes: Option<EventAttributes>,
        timestamp: Option<DateTime<Utc>>,
    ) {
        let event = Event {
            timestamp: timestamp.unwrap_or_else(Utc::now),
            message,
            event_type: EventType::ProcessStatus,
            process_type: ProcessType::Pipeline,
            process_status,
            attributes,
            // NOTE: not a fan of constant cloning so would look for an alt
            run_name: self.run_name.clone(),
            run_id: self.run_id.clone(),
            pipeline_name: self.pipeline_name.clone(),
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
    use crate::event::attributes::EventAttributes;
    use crate::event::event::ProcessStatus;
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
        assert_eq!(event.message, message);
        assert_eq!(event.event_type, EventType::ProcessStatus);
        assert_eq!(event.process_type, ProcessType::Pipeline);
        assert_eq!(event.process_status, ProcessStatus::ToolExecution);
        assert!(matches!(
            event.attributes.clone().unwrap(),
            EventAttributes::Other(_)
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
        assert_eq!(ProcessStatus::FinishedRun.as_str(), "finished_run");
        assert_eq!(ProcessStatus::ToolExecution.as_str(), "tool_execution");
        assert_eq!(ProcessStatus::MetricEvent.as_str(), "metric_event");
        assert_eq!(ProcessStatus::TestEvent.as_str(), "test_event");
    }

    #[test]
    fn test_record_test_event() {
        let mut recorder = EventRecorder::default();
        let message = "Test event for testing".to_string();
        let attributes = Some(EventAttributes::Other(json!({"test_key": "test_value"})));

        recorder.record_event(
            ProcessStatus::TestEvent,
            message.clone(),
            attributes.clone(),
            None,
        );

        assert_eq!(recorder.len(), 1);

        let event = &recorder.get_events()[0];
        assert_eq!(event.message, message);
        assert_eq!(event.event_type, EventType::ProcessStatus);
        assert_eq!(event.process_type, ProcessType::Pipeline);
        assert_eq!(event.process_status, ProcessStatus::TestEvent);
        assert!(matches!(
            event.attributes.clone().unwrap(),
            EventAttributes::Other(_)
        ));
    }
}
