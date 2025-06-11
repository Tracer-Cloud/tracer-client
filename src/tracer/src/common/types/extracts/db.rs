use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};

use anyhow::Context;

use crate::common::types::event::attributes::process::ProcessProperties;
use crate::common::types::event::{attributes::EventAttributes, Event};
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
                EventAttributes::Process(ProcessProperties::ShortLived(_)) => {
                    cpu_usage = Some(0.0);
                    mem_used = Some(0.0);
                }
                EventAttributes::SystemMetric(m) => {
                    cpu_usage = Some(m.system_cpu_utilization);
                    mem_used = Some(m.system_memory_used as f64);
                }
                EventAttributes::SystemProperties(p) => {
                    ec2_cost_per_hour = p.ec2_cost_per_hour;

                    // Properly flatten and assign to `resource_attributes`
                    let mut flat = Map::new();
                    crate::common::utils::flatten_with_prefix(
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
            attributes = crate::common::utils::flatten_event_attributes(&event)?;
        }

        let tags = event.tags.clone();

        Ok(EventInsert {
            timestamp: event.timestamp,
            body: event.body,
            severity_text: event.severity_text,
            severity_number: event.severity_number.map(|v| v as i16),
            trace_id: trace_id.or_else(|| event.run_id.clone()),
            span_id: event.span_id,

            source_type: "tracer-daemon".to_string(),
            instrumentation_version: option_env!("CARGO_PKG_VERSION").map(str::to_string),
            instrumentation_type: Some("TRACER_DAEMON".to_string()),
            environment: tags.as_ref().and_then(|t| t.environment.clone()),
            pipeline_type: tags.as_ref().and_then(|t| t.pipeline_type.clone()),
            user_operator: tags.as_ref().and_then(|t| t.user_operator.clone()),
            organization_id: tags.as_ref().and_then(|t| t.organization_id.clone()),
            department: tags.as_ref().map(|t| t.department.clone()),

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
            tags: serde_json::to_value(&tags).context("Failed to serialize tags")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::common::types::event::attributes::process::{
        ProcessProperties, ShortProcessProperties,
    };
    use crate::common::types::event::attributes::EventAttributes;
    use crate::common::types::event::{Event, EventType, ProcessStatus, ProcessType};
    use crate::common::types::extracts::db::EventInsert;
    use chrono::Utc;
    use std::convert::TryFrom;

    #[test]
    fn test_event_insert_short_lived_process() {
        let now = Utc::now();
        let short_lived_props = ShortProcessProperties {
            tool_name: "test_process".to_string(),
            tool_pid: "12345".to_string(),
            tool_parent_pid: "1".to_string(),
            tool_binary_path: "/usr/bin/test_process".to_string(),
            start_timestamp: now.to_rfc3339(),
        };

        let event = Event {
            timestamp: now,
            body: "Test Event".to_string(),
            severity_text: None,
            severity_number: None,
            span_id: None,
            trace_id: Some("trace-id-123".to_string()),
            run_id: Some("test-run-id".to_string()),
            run_name: Some("test_run".to_string()),
            pipeline_name: Some("test_pipeline".to_string()),
            event_type: EventType::ProcessStatus,
            process_type: ProcessType::Pipeline,
            process_status: ProcessStatus::ToolExecution,
            attributes: Some(EventAttributes::Process(ProcessProperties::ShortLived(
                Box::new(short_lived_props),
            ))),
            tags: None,
        };

        // Convert the event to EventInsert
        let event_insert = EventInsert::try_from(event).unwrap();

        assert_eq!(
            event_insert.cpu_usage,
            Some(0.0),
            "CPU usage should be 0.0 for ShortLived processes"
        );
        assert_eq!(
            event_insert.mem_used,
            Some(0.0),
            "Memory used should be 0.0 for ShortLived processes"
        );
    }
}
