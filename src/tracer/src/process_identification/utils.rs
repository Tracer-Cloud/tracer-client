use crate::process_identification::types::event::attributes::process::ProcessProperties;
use crate::process_identification::types::event::{attributes::EventAttributes, Event};
use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::fs::OpenOptions;
use std::io::Write;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::{error, info};

/// Flattens the `attributes` field of an event with prefixing, e.g.:
/// `process.tool_name` or `system_properties.ec2_cost_per_hour`
pub fn flatten_event_attributes(event: &Event) -> Result<Value> {
    let mut map = Map::new();

    let attrs = event
        .attributes
        .as_ref()
        .context("Missing event attributes")?;

    let (prefix, json) = match attrs {
        EventAttributes::Process(ProcessProperties::Full(p)) => {
            ("process", serde_json::to_value(p)?)
        }
        EventAttributes::CompletedProcess(p) => ("process", serde_json::to_value(p)?),
        EventAttributes::SystemMetric(p) => ("system_metric", serde_json::to_value(p)?),
        EventAttributes::SystemProperties(_) => return Ok(Value::Object(map)),
        EventAttributes::ProcessDatasetStats(p) => {
            ("processed_dataset_stats", serde_json::to_value(p)?)
        }
        EventAttributes::Syslog(p) => ("syslog", serde_json::to_value(p)?),

        EventAttributes::ContainerEvents(p) => ("containers", serde_json::to_value(p)?),
        EventAttributes::TaskMatch(p) => ("task_match", serde_json::to_value(p)?),
    };

    flatten_with_prefix(prefix, &json, &mut map);

    Ok(Value::Object(map))
}

pub fn flatten_with_prefix(prefix: &str, val: &Value, out: &mut Map<String, Value>) {
    match val {
        Value::Object(obj) => {
            for (k, v) in obj {
                let new_key = format!("{}.{}", prefix, k);
                flatten_with_prefix(&new_key, v, out);
            }
        }
        Value::Array(arr) => {
            // Optionally, serialize arrays as JSON strings
            out.insert(
                prefix.to_string(),
                Value::String(serde_json::to_string(arr).unwrap()),
            );
        }
        _ => {
            out.insert(prefix.to_string(), val.clone());
        }
    }
}

pub fn log_matched_process(trigger: &ProcessStartTrigger, matched_rule: &str, is_matched: bool) {
    let matched_string = if is_matched { "MATCHED" } else { "NOT MATCHED" };

    let log_line = format!(
        "{} | {} | {} | {} | {}\n\n\n",
        trigger.pid, trigger.comm, trigger.command_string, matched_string, matched_rule,
    );
    info!(log_line);
    if let Err(e) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&TRACER_WORK_DIR.process_matches_file)
        .and_then(|mut file| file.write_all(log_line.as_bytes()))
    {
        error!("Failed to write match log: {}", e);
    }
}
