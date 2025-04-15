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

impl ProcessStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessStatus::NewRun => "new_run",
            ProcessStatus::FinishedRun => "finished_run",
            ProcessStatus::ToolExecution => "tool_execution",
            ProcessStatus::FinishedToolExecution => "finished_tool_execution",
            ProcessStatus::MetricEvent => "metric_event",
            ProcessStatus::SyslogEvent => "syslog_event",
            ProcessStatus::ToolMetricEvent => "tool_metric_event",
            ProcessStatus::TestEvent => "test_event", // Handle TestEvent
            ProcessStatus::RunStatusMessage => "run_status_message",
            ProcessStatus::Alert => "alert",
            ProcessStatus::DataSamplesEvent => "datasets_in_process",
            ProcessStatus::NextflowLogEvent => "nextflow_log_event",
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
