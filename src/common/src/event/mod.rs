pub mod attributes;

use crate::event::attributes::EventAttributes;
use crate::pipeline_tags::PipelineTags;
use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use typed_builder::TypedBuilder;
use uuid::Uuid;

use anyhow::Context;

use std::convert::TryFrom;

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

impl TryFrom<Event> for EventInsert {
    type Error = anyhow::Error;

    fn try_from(event: Event) -> anyhow::Result<Self> {
        let mut attributes = json!({});
        let mut resource_attributes = json!({});
        let mut job_id = None;
        let mut parent_job_id = None;
        let mut child_job_ids = None;
        let mut workflow_engine = None;
        let mut cpu_usage = None;
        let mut mem_used = None;
        let mut ec2_cost_per_hour = None;
        let mut processed_dataset = None;

        if let Some(attr) = &event.attributes {
            match attr {
                EventAttributes::Process(p) => {
                    cpu_usage = Some(p.process_cpu_utilization);
                    mem_used = Some(p.process_memory_usage as f64);
                    job_id = p.job_id.clone();
                    attributes = serde_json::to_value(p)
                        .context("Failed to serialize Process attributes")?;
                }
                EventAttributes::SystemMetric(m) => {
                    cpu_usage = Some(m.system_cpu_utilization);
                    mem_used = Some(m.system_memory_used as f64);
                    attributes = serde_json::to_value(m)
                        .context("Failed to serialize SystemMetric attributes")?;
                }
                EventAttributes::SystemProperties(p) => {
                    ec2_cost_per_hour = p.ec2_cost_per_hour;
                    resource_attributes =
                        serde_json::to_value(p).context("Failed to serialize SystemProperties")?;
                }
                EventAttributes::ProcessDatasetStats(d) => {
                    processed_dataset = Some(d.total as i32);
                    attributes = serde_json::to_value(d)
                        .context("Failed to serialize ProcessDatasetStats")?;
                }
                EventAttributes::NextflowLog(n) => {
                    parent_job_id = n.session_uuid.clone();
                    child_job_ids = n.jobs_ids.clone();
                    workflow_engine = Some("nextflow".to_string());
                    attributes =
                        serde_json::to_value(n).context("Failed to serialize NextflowLog")?;
                }
                EventAttributes::Syslog(s) => {
                    attributes =
                        serde_json::to_value(s).context("Failed to serialize Syslog attributes")?;
                }
                _ => {}
            }
        }

        let tags = event.tags.clone();

        Ok(EventInsert {
            event_timestamp: event.timestamp,
            body: event.body,
            severity_text: event.severity_text,
            severity_number: event.severity_number.map(|v| v as i16),
            trace_id: event.trace_id.or_else(|| event.run_id.clone()),
            span_id: event.span_id,

            source_type: "tracer-daemon".into(),
            instrumentation_version: option_env!("CARGO_PKG_VERSION").map(|s| s.to_string()),
            instrumentation_type: Some("TRACER_DAEMON".into()),
            environment: tags.clone().map(|t| t.environment),
            pipeline_type: tags.clone().map(|t| t.pipeline_type),
            user_operator: tags.clone().map(|t| t.user_operator),
            organization_id: tags.clone().map(|t| t.organization_id).unwrap_or_default(),
            department: tags.clone().map(|t| t.department),

            run_id: event.run_id.unwrap_or_default(),
            run_name: event.run_name.unwrap_or_default(),
            pipeline_name: event.pipeline_name.unwrap_or_default(),
            job_id,
            parent_job_id,
            child_job_ids,
            workflow_engine,

            ec2_cost_per_hour,
            cpu_usage,
            mem_used,
            processed_dataset,
            process_status: event.process_status.to_string(),

            attributes,
            resource_attributes,
            tags: serde_json::to_value(tags).context("Failed to serialize tags")?,
        })
    }
}
