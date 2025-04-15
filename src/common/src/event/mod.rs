pub mod attributes;
pub mod otel;
use crate::event::attributes::EventAttributes;
use crate::pipeline_tags::PipelineTags;
use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use otel::OtelLog;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

// TODO: would be removed in next pr
pub struct EventInsert {
    pub event_timestamp: DateTime<Utc>,
    pub body: String,
    pub severity_text: Option<String>,
    pub severity_number: Option<i16>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,

    pub source_type: String,
    pub instrumentation_version: Option<String>,
    pub instrumentation_type: Option<String>,
    pub environment: Option<String>,
    pub pipeline_type: Option<String>,
    pub user_operator: Option<String>,
    pub organization_id: Option<String>,
    pub department: Option<String>,

    pub run_id: String,
    pub run_name: String,
    pub pipeline_name: String,
    pub job_id: Option<String>,
    pub parent_job_id: Option<String>,
    pub child_job_ids: Option<Vec<String>>,
    pub workflow_engine: Option<String>,

    pub ec2_cost_per_hour: Option<f64>,
    pub cpu_usage: Option<f32>,
    pub mem_used: Option<f64>,
    pub processed_dataset: Option<i32>,
    pub process_status: String,

    pub attributes: Value,
    pub resource_attributes: Value,
    pub tags: Value,
}

impl EventInsert {
    pub fn try_new(
        log: OtelLog,
        run_name: String,
        run_id: String,
        pipeline_name: String,
        process_status: String,
    ) -> anyhow::Result<Self> {
        Ok(EventInsert {
            event_timestamp: log.timestamp,
            body: log.body,
            severity_text: log.severity_text,
            severity_number: log.severity_number.map(|v| v as i16),
            trace_id: log.trace_id,
            span_id: log.span_id,

            source_type: log.source_type,
            instrumentation_version: log.instrumentation_version,
            instrumentation_type: log.instrumentation_type,
            environment: log.environment,
            pipeline_type: log.pipeline_type,
            user_operator: log.user_operator,
            organization_id: log.organization_id,
            department: log.department,

            run_id,
            run_name,
            pipeline_name,
            job_id: log.job_id,
            parent_job_id: log.parent_job_id,
            child_job_ids: log.child_job_ids,
            workflow_engine: log.workflow_engine,

            ec2_cost_per_hour: log.ec2_cost_per_hour,
            cpu_usage: log.cpu_usage,
            mem_used: log.mem_used,
            processed_dataset: log.processed_dataset,
            process_status,

            attributes: log.attributes.unwrap_or_else(|| serde_json::json!({})),
            resource_attributes: log
                .resource_attributes
                .unwrap_or_else(|| serde_json::json!({})),
            tags: serde_json::to_value(log.tags).unwrap_or_else(|_| serde_json::json!({})),
        })
    }
}
