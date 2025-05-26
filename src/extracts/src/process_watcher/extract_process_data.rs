use crate::process_watcher::process_utils::process_status_to_string;
use chrono::{DateTime, Utc};
use std::path::Path;
use sysinfo::{Process};
use tracer_common::types::event::attributes::process::{FullProcessProperties, ProcessProperties};
use tracing::debug;

pub struct ExtractProcessData {}

impl ExtractProcessData {
    /// Extracts environment variables related to containerization, jobs, and tracing
    pub fn extract_process_environment_variables(
        proc: &Process,
    ) -> (Option<String>, Option<String>, Option<String>) {
        let mut container_id = None;
        let mut job_id = None;
        let mut trace_id = None;

        // Try to read environment variables
        for env_var in proc.environ() {
            if let Some((key, value)) = env_var.split_once('=') {
                match key {
                    "AWS_BATCH_JOB_ID" => job_id = Some(value.to_string()),
                    "HOSTNAME" => container_id = Some(value.to_string()),
                    "TRACER_TRACE_ID" => trace_id = Some(value.to_string()),
                    _ => continue,
                }
            }
        }

        (container_id, job_id, trace_id)
    }

    pub async fn gather_process_data(
        proc: &Process,
        display_name: String,
        process_start_time: DateTime<Utc>,
    ) -> ProcessProperties {
        debug!("Gathering process data for {}", display_name);

        let (container_id, job_id, trace_id) =
            ExtractProcessData::extract_process_environment_variables(proc);

        let working_directory = proc.cwd().map(|p| p.to_string_lossy().to_string());

        let process_run_time = (Utc::now() - process_start_time).num_milliseconds().max(0) as u64;

        ProcessProperties::Full(Box::new(FullProcessProperties {
            tool_name: display_name,
            tool_pid: proc.pid().as_u32().to_string(),
            tool_parent_pid: proc.parent().unwrap_or(0.into()).to_string(),
            tool_binary_path: proc
                .exe()
                .unwrap_or_else(|| Path::new(""))
                .as_os_str()
                .to_str()
                .unwrap_or("")
                .to_string(),
            tool_cmd: proc.cmd().join(" "),
            start_timestamp: process_start_time.to_rfc3339(),
            process_cpu_utilization: proc.cpu_usage(),
            process_run_time, // time in milliseconds
            process_disk_usage_read_total: proc.disk_usage().total_read_bytes,
            process_disk_usage_write_total: proc.disk_usage().total_written_bytes,
            process_disk_usage_read_last_interval: proc.disk_usage().read_bytes,
            process_disk_usage_write_last_interval: proc.disk_usage().written_bytes,
            process_memory_usage: proc.memory(),
            process_memory_virtual: proc.virtual_memory(),
            process_status: process_status_to_string(&proc.status()),
            container_id,
            job_id,
            working_directory,
            trace_id,
        }))
    }
}

mod tests {
    // add there tests for these functions
}