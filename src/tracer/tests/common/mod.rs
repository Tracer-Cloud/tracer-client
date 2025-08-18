use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracer_ebpf::ebpf_trigger::Trigger;

pub const DUMMY_PID: usize = 0;
pub const DUMMY_PPID: usize = 1;

/// Creates a new `ProcessStartTrigger`. If the command is
/// `<file>.py``, changes the command to `python <file>.py`.
/// Use dummy PID and PPID values.
pub fn new_process_start_trigger(cmd: &str) -> Trigger {
    // change command for executable scripts
    let cmd = if cmd.contains(".py") {
        format!("python {}", cmd)
    } else {
        cmd.to_string()
    };
    Trigger::ProcessStart(ProcessStartTrigger::from_command_string(
        DUMMY_PID, DUMMY_PPID, &cmd,
    ))
}

/// Structure for process info loaded from a JSON file.
/// Each process has a vec of test command sto try and match, and a
/// vec of commands that are expected to be matched.
#[derive(Debug, Deserialize, Serialize)]
pub struct ProcessInfo {
    pub process_name: String,
    pub test_commands: Vec<Vec<String>>,
    pub match_commands: Option<Vec<String>>,
    pub pattern: String,
}

impl ProcessInfo {
    pub fn from_file(file_path: &str) -> Result<Vec<Self>> {
        let content = std::fs::read_to_string(file_path)?;
        let processes = serde_json::from_str(&content)?;
        Ok(processes)
    }

    pub fn match_commands(&self) -> Vec<String> {
        if let Some(match_commands) = &self.match_commands {
            match_commands.to_owned()
        } else {
            let match_command = self.pattern.split(" ").next().unwrap().trim();
            // Removes regex characters from the first element in `self.pattern` to turn it into a valid
            // path. Currontly only removes leading '^' and strips whitespace.
            let match_command = if let Some(stripped) = match_command.strip_prefix('^') {
                stripped
            } else {
                match_command
            };
            vec![match_command.replace("\\", "")]
        }
    }

    pub fn tool_names(&self) -> impl Iterator<Item = String> {
        let match_commands = self.match_commands();
        match_commands
            .into_iter()
            .map(|c| c.split("/").last().unwrap().to_owned())
    }
}
