use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};

use anyhow::Context;

use crate::types::event::attributes::process::ProcessProperties;
use crate::types::event::{attributes::EventAttributes, Event};
use serde::Serialize;
use std::convert::TryFrom;

#[derive(Serialize, Clone, Debug)]
pub struct EventInsert {
    pub timestamp: DateTime<Utc>,
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

    pub event_type: String,
    pub process_type: String,

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
        let mut trace_id = None;
        let parent_job_id = None;
        let child_job_ids = None;
        let workflow_engine = None;
        let mut cpu_usage = None;
        let mut mem_used = None;
        let mut ec2_cost_per_hour = None;
        let mut processed_dataset = None;

        if let Some(attr) = &event.attributes {
            match attr {
                EventAttributes::Process(ProcessProperties::Full(p)) => {
                    cpu_usage = Some(p.process_cpu_utilization);
                    mem_used = Some(p.process_memory_usage as f64);
                    job_id = p.job_id.clone();
                    trace_id = p.trace_id.clone();
                }
                EventAttributes::Process(ProcessProperties::ShortLived(_)) => {}
                EventAttributes::SystemMetric(m) => {
                    cpu_usage = Some(m.system_cpu_utilization);
                    mem_used = Some(m.system_memory_used as f64);
                }
                EventAttributes::SystemProperties(p) => {
                    ec2_cost_per_hour = p.ec2_cost_per_hour;

                    // Properly flatten and assign to `resource_attributes`
                    let mut flat = Map::new();
                    crate::utils::flatten_with_prefix(
                        "system_properties",
                        &serde_json::to_value(p).context("serialize system_properties")?,
                        &mut flat,
                    );
                    resource_attributes = Value::Object(flat);
                }
                EventAttributes::ProcessDatasetStats(d) => {
                    processed_dataset = Some(d.total as i32);
                    trace_id = d.trace_id.clone();
                }
                _ => {}
            }

            // Flatten main attributes using utility
            attributes = crate::utils::flatten_event_attributes(&event)?;
        }

        let tags = event.tags.clone();

        Ok(EventInsert {
            timestamp: event.timestamp,
            body: event.body,
            severity_text: event.severity_text,
            severity_number: event.severity_number.map(|v| v as i16),
            trace_id: trace_id.or_else(|| event.run_id.clone()),
            span_id: event.span_id,

            source_type: "tracer-daemon".into(),
            instrumentation_version: option_env!("CARGO_PKG_VERSION").map(|s| s.to_string()),
            instrumentation_type: Some("TRACER_DAEMON".into()),
            environment: tags.clone().map(|t| t.environment),
            pipeline_type: tags.clone().map(|t| t.pipeline_type),
            user_operator: tags.clone().map(|t| t.user_operator),
            organization_id: tags.clone().map(|t| t.organization_id).unwrap_or_default(),
            department: tags.clone().map(|t| t.department),

            event_type: event.event_type.to_string(),
            process_type: event.process_type.to_string(),

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
