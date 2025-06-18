use crate::common::types::event::attributes::process::{ProcessProperties, ShortProcessProperties};
use chrono::Utc;
use itertools::Itertools;
use std::process::Command;
use sysinfo::ProcessStatus;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

pub fn process_status_to_string(status: &ProcessStatus) -> String {
    match status {
        ProcessStatus::Run => "Run".to_string(),
        ProcessStatus::Sleep => "Sleep".to_string(),
        ProcessStatus::Idle => "Idle".to_string(),
        ProcessStatus::Zombie => "Zombie".to_string(),
        ProcessStatus::Stop => "Stop".to_string(),
        ProcessStatus::Parked => "Parked".to_string(),
        ProcessStatus::Tracing => "Tracing".to_string(),
        ProcessStatus::Dead => "Dead".to_string(),
        ProcessStatus::UninterruptibleDiskSleep => "Uninterruptible Disk Sleep".to_string(),
        ProcessStatus::Waking => "Waking".to_string(),
        ProcessStatus::LockBlocked => "Lock Blocked".to_string(),
        _ => "Unknown".to_string(),
    }
}

/// Creates properties for a short-lived process that wasn't found in the system
pub fn create_short_lived_process_properties(
    process: &ProcessStartTrigger,
    display_name: String,
) -> ProcessProperties {
    ProcessProperties::ShortLived(Box::new(ShortProcessProperties {
        tool_name: display_name,
        tool_pid: process.pid.to_string(),
        tool_parent_pid: process.ppid.to_string(),
        tool_binary_path: process.file_name.clone(), // TODO WTF
        start_timestamp: Utc::now().to_rfc3339(),
        tool_args: process.argv.iter().join(" "),
        tool_cmd: process.comm.clone(),
    }))
}

// Simple command line parser (handles basic cases)
pub fn parse_command_line(cmd_line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut in_quotes = false;
    let command_line_characters = cmd_line.chars().peekable();

    for character in command_line_characters {
        match character {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' if !in_quotes => {
                if !current_arg.is_empty() {
                    args.push(current_arg.clone());
                    current_arg.clear();
                }
            }
            _ => {
                current_arg.push(character);
            }
        }
    }

    if !current_arg.is_empty() {
        args.push(current_arg);
    }

    args
}

// Helper function to get command line arguments using ps
pub fn get_process_argv(pid: i32) -> Vec<String> {
    match Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
    {
        Ok(output) if output.status.success() => {
            if let Ok(command_line) = String::from_utf8(output.stdout) {
                let command_line = command_line.trim();
                if !command_line.is_empty() {
                    // Simple parsing - split by spaces but handle quoted args
                    return parse_command_line(command_line);
                }
            }
        }
        _ => {}
    }
    vec![]
}
