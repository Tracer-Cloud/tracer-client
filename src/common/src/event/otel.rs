use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{attributes::EventAttributes, Event};

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

impl From<Event> for OtelLog {
    fn from(b: Event) -> Self {
        let mut attributes = json!({});
        let mut resource_attributes = json!({});

        let (mut cpu_usage, mut mem_used, mut ec2_cost_per_hour, mut processed_dataset) =
            (None, None, None, None);
        let (mut job_id, mut parent_job_id, mut child_job_ids, mut workflow_engine) =
            (None, None, None, None);

        if let Some(attr) = &b.attributes {
            match attr {
                EventAttributes::Process(p) => {
                    cpu_usage = Some(p.process_cpu_utilization);
                    mem_used = Some(p.process_memory_usage as f64);
                    job_id = p.job_id.clone();
                    attributes = serde_json::to_value(p).unwrap_or_default();
                }
                EventAttributes::SystemMetric(m) => {
                    cpu_usage = Some(m.system_cpu_utilization);
                    mem_used = Some(m.system_memory_used as f64);
                    attributes = serde_json::to_value(m).unwrap_or_default();
                }
                EventAttributes::SystemProperties(p) => {
                    ec2_cost_per_hour = p.ec2_cost_per_hour;
                    resource_attributes = serde_json::to_value(p).unwrap_or_default();
                }
                EventAttributes::ProcessDatasetStats(d) => {
                    processed_dataset = Some(d.total as i32);
                    attributes = serde_json::to_value(d).unwrap_or_default();
                }
                EventAttributes::NextflowLog(n) => {
                    parent_job_id = n.session_uuid.clone();
                    child_job_ids = n.jobs_ids.clone();
                    workflow_engine = Some("nextflow".to_string());
                    attributes = serde_json::to_value(n).unwrap_or_default();
                }
                EventAttributes::Syslog(s) => {
                    attributes = serde_json::to_value(s).unwrap_or_default();
                }
                _ => {}
            }
        }

        OtelLog {
            timestamp: b.timestamp,
            body: b.message,
            severity_text: None,
            severity_number: None,
            trace_id: None,
            span_id: None,

            source_type: "tracer-daemon".to_string(),
            instrumentation_version: option_env!("CARGO_PKG_VERSION").map(|s| s.to_string()),
            instrumentation_type: Some("TRACER_DAEMON".to_string()),
            environment: b.tags.clone().map(|t| t.environment),
            department: b.tags.clone().map(|t| t.department),
            organization_id: b
                .tags
                .clone()
                .map(|t| t.organization_id)
                .unwrap_or_default(),
            user_operator: b.tags.clone().map(|t| t.user_operator),
            pipeline_type: b.tags.clone().map(|t| t.pipeline_type),

            run_id: b.run_id,
            run_name: b.run_name,
            pipeline_name: b.pipeline_name,

            job_id,
            parent_job_id,
            child_job_ids,
            workflow_engine,

            ec2_cost_per_hour,
            cpu_usage,
            mem_used,
            processed_dataset,
            process_status: b.process_status,

            attributes: Some(attributes),
            resource_attributes: Some(resource_attributes),
            tags: b.tags,
        }
    }
}

impl From<OtelLog> for  {
    fn from(o: OtelLog) -> Self {
        EventInsert {
            event_timestamp: o.timestamp,
            body: o.body,
            severity_text: o.severity_text,
            severity_number: o.severity_number.map(|v| v as i16),
            trace_id: o.trace_id,
            span_id: o.span_id,

            source_type: o.source_type,
            instrumentation_version: o.instrumentation_version,
            instrumentation_type: o.instrumentation_type,
            environment: o.environment,
            pipeline_type: o.pipeline_type,
            user_operator: o.user_operator,
            organization_id: o.organization_id,
            department: o.department,

            run_id: o.run_id.unwrap_or_default(),
            run_name: o.run_name.unwrap_or_default(),
            pipeline_name: o.pipeline_name.unwrap_or_default(),
            job_id: o.job_id,
            parent_job_id: o.parent_job_id,
            child_job_ids: o.child_job_ids,
            workflow_engine: o.workflow_engine,

            ec2_cost_per_hour: o.ec2_cost_per_hour,
            cpu_usage: o.cpu_usage,
            mem_used: o.mem_used,
            processed_dataset: o.processed_dataset,
            process_status: o.process_status.as_str().to_string(),

            attributes: o.attributes.unwrap_or_else(|| json!({})),
            resource_attributes: o.resource_attributes.unwrap_or_else(|| json!({})),
            tags: serde_json::to_value(o.tags).unwrap_or_else(|_| json!({})),
        }
    }
}
