use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracer_ebpf::ebpf_trigger::Trigger;

pub const DUMMY_PID: usize = 1;
pub const DUMMY_PPID: usize = 0;

pub fn new_process_start_trigger(cmd: &str, path: &str) -> Trigger {
    Trigger::ProcessStart(ProcessStartTrigger {
        pid: DUMMY_PID,
        ppid: DUMMY_PPID,
        comm: path.split("/").last().unwrap().to_string(),
        argv: cmd.split_whitespace().map(String::from).collect(),
        file_name: path.to_string(),
        started_at: Utc::now(),
    })
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProcessInfo {
    pub process_name: String,
    pub test_commands: Vec<Vec<String>>,
    pub pattern: String,
    pub tool_name: Option<String>,
}

impl ProcessInfo {
    pub fn from_file(file_path: &str) -> Result<Vec<Self>> {
        let content = std::fs::read_to_string(file_path)?;
        let processes = serde_json::from_str(&content)?;
        Ok(processes)
    }

    /// Removes regex characters from the first element in `self.pattern` to turn it into a valid
    /// path. Currontly only removes leading '^' and strips whitespace.
    pub fn path(&self) -> &str {
        let pattern = if self.pattern.starts_with('^') {
            &self.pattern[1..]
        } else {
            &self.pattern
        };
        pattern.split(" ").next().unwrap().trim()
    }

    pub fn tool_name(&self) -> &str {
        self.tool_name
            .as_deref()
            .unwrap_or_else(|| self.path().split("/").last().unwrap())
    }
}
