use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::pipeline_tags::PipelineTags;

use super::{attributes::EventAttributes, Event, EventInsert, ProcessStatus};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OtelLog {
    pub timestamp: DateTime<Utc>,
    pub body: String,
    pub severity_text: Option<String>,
    pub severity_number: Option<u8>,
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

    pub run_id: Option<String>,
    pub run_name: Option<String>,
    pub pipeline_name: Option<String>,

    pub job_id: Option<String>,
    pub parent_job_id: Option<String>,
    pub child_job_ids: Option<Vec<String>>,
    pub workflow_engine: Option<String>,

    pub ec2_cost_per_hour: Option<f64>,
    pub cpu_usage: Option<f32>,
    pub mem_used: Option<f64>,
    pub processed_dataset: Option<i32>,
    pub process_status: ProcessStatus,

    pub attributes: Option<Value>,
    pub resource_attributes: Option<Value>,
    pub tags: Option<PipelineTags>,
}
