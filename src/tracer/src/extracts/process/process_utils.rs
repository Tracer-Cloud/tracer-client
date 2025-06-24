use crate::process_identification::types::event::attributes::process::{
    FullProcessProperties, ProcessProperties,
};
use chrono::Utc;
use itertools::Itertools;
use shlex;
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
pub fn create_short_lived_process_object(
    process: &ProcessStartTrigger,
    display_name: String,
) -> ProcessProperties {
    ProcessProperties::Full(Box::new(FullProcessProperties {
        tool_name: display_name,
        tool_pid: process.pid.to_string(),
        tool_parent_pid: process.ppid.to_string(),
        tool_binary_path: "".to_string(), // TODO WTF
        tool_cmd: process.comm.clone(),
        tool_args: process.argv.iter().join(" "),
        start_timestamp: Utc::now().to_rfc3339(),
        process_cpu_utilization: 0.0,
        process_run_time: 0,
        process_disk_usage_read_total: 0,
        process_disk_usage_write_total: 0,
        process_disk_usage_read_last_interval: 0,
        process_disk_usage_write_last_interval: 0,
        process_memory_usage: 0,
        process_memory_virtual: 0,
        process_status: process_status_to_string(&ProcessStatus::Unknown(0)),
        container_id: None,
        job_id: None,
        working_directory: None,
        trace_id: None,
    }))
}

// Simple command line parser (handles basic cases)
// pub fn parse_command_line(cmd_line: &str) -> Vec<String> {
//     let mut args = Vec::new();
//     let mut current_arg = String::new();
//     let mut in_quotes = false;
//     let command_line_characters = cmd_line.chars().peekable();

//     for character in command_line_characters {
//         match character {
//             '"' => {
//                 in_quotes = !in_quotes;
//             }
//             ' ' if !in_quotes => {
//                 if !current_arg.is_empty() {
//                     args.push(current_arg.clone());
//                     current_arg.clear();
//                 }
//             }
//             _ => {
//                 current_arg.push(character);
//             }
//         }
//     }

//     if !current_arg.is_empty() {
//         args.push(current_arg);
//     }

//     args
// }

// Helper function to get command line arguments using ps
pub fn get_process_argv(pid: i32) -> Vec<String> {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .ok()
                .and_then(|command_line| {
                    let command_line = command_line.trim();
                    if !command_line.is_empty() {
                        shlex::split(command_line)
                    } else {
                        None
                    }
                })
        })
        .unwrap_or_default()
}
