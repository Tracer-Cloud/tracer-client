pub mod attributes;
use crate::event::attributes::EventAttributes;
use crate::pipeline_tags::PipelineTags;
use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    ProcessStatus,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessType {
    Pipeline,
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
    #[serde(rename = "datasets_in_process")]
    DataSamplesEvent,
    TestEvent, // Added TestEvent variant
    NextflowLogEvent,
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
            ProcessStatus::DataSamplesEvent => write!(f, "datasets_in_process"),
            ProcessStatus::TestEvent => write!(f, "test_event"),
            ProcessStatus::NextflowLogEvent => write!(f, "nextflow_log_event"),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Event {
    #[serde(with = "ts_seconds")]
    pub timestamp: DateTime<Utc>,
    pub message: String,

    pub event_type: EventType,
    pub process_type: ProcessType,
    pub process_status: ProcessStatus,

    pub pipeline_name: Option<String>,
    pub run_name: Option<String>,
    pub run_id: Option<String>,
    pub attributes: Option<EventAttributes>,
    pub tags: Option<PipelineTags>,
}
