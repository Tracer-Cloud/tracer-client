pub mod attributes;

use super::event::attributes::EventAttributes;
use super::pipeline_tags::PipelineTags;
use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use typed_builder::TypedBuilder;
use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    ProcessStatus,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            &EventType::ProcessStatus => write!(f, "process_status"),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessType {
    Pipeline,
}

impl std::fmt::Display for ProcessType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            &ProcessType::Pipeline => write!(f, "pipeline"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    NewRun,
    FinishedRun,
    ToolExecution,
    FinishedToolExecution,
    ToolMetricEvent,
    MetricEvent,
    SyslogEvent,
    RunStatusMessage,
    Alert,
    TestEvent,
    ContainerExecution,
    ContainerTermination,
    TaskMatch,
    FileOpened,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::NewRun => write!(f, "new_run"),
            ProcessStatus::FinishedRun => write!(f, "finished_run"),
            ProcessStatus::ToolExecution => write!(f, "tool_execution"),
            ProcessStatus::FinishedToolExecution => write!(f, "finished_tool_execution"),
            ProcessStatus::ToolMetricEvent => write!(f, "tool_metric_event"),
            ProcessStatus::MetricEvent => write!(f, "metric_event"),
            ProcessStatus::SyslogEvent => write!(f, "syslog_event"),
            ProcessStatus::RunStatusMessage => write!(f, "run_status_message"),
            ProcessStatus::Alert => write!(f, "alert"),
            ProcessStatus::FileOpened => write!(f, "file_opened"),
            ProcessStatus::TestEvent => write!(f, "test_event"),
            ProcessStatus::ContainerExecution => write!(f, "container_execution"),
            ProcessStatus::ContainerTermination => write!(f, "container_termination"),
            ProcessStatus::TaskMatch => write!(f, "task_match"),
        }
    }
}

fn default_span_id() -> Option<String> {
    Some(Uuid::new_v4().to_string())
}

#[derive(Serialize, Deserialize, Debug, Clone, TypedBuilder)]
#[builder(field_defaults(default))]
pub struct Event {
    #[serde(with = "ts_seconds")]
    pub timestamp: DateTime<Utc>,

    #[builder(setter(into))]
    pub body: String,

    #[builder(default = EventType::ProcessStatus)]
    pub event_type: EventType,

    #[builder(default = ProcessType::Pipeline)]
    pub process_type: ProcessType,

    #[builder(default = ProcessStatus::TestEvent)]
    pub process_status: ProcessStatus,

    pub pipeline_name: Option<String>,
    pub run_name: Option<String>,
    pub run_id: Option<String>,
    pub attributes: Option<EventAttributes>,
    pub tags: Option<PipelineTags>,

    pub severity_text: Option<String>,
    pub severity_number: Option<u8>,
    pub trace_id: Option<String>,

    #[builder(default = default_span_id())]
    pub span_id: Option<String>,
}
