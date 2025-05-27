use chrono::Utc;
use sysinfo::ProcessStatus;
use tokio::sync::{ RwLockReadGuard};
use tracer_common::types::ebpf_trigger::ProcessStartTrigger;
use tracer_common::types::event::attributes::process::{ProcessProperties, ShortProcessProperties};
use crate::process_watcher::handler::process::process_manager::ProcessState;

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
        tool_binary_path: process.file_name.clone(),
        start_timestamp: Utc::now().to_rfc3339(),
    }))
}

pub async fn get_targets_len(process_state: RwLockReadGuard<'_, ProcessState>) -> usize {
        process_state
        .get_monitoring()
        .values()
        .map(|processes| processes.len())
        .sum()
}
