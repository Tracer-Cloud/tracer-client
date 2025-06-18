use chrono::Utc;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

pub const DUMMY_PID: usize = 1;
pub const DUMMY_PPID: usize = 0;
pub const PATH_SEPARATOR: &str = "/";

pub fn new_process_start_trigger(cmd: &str, path: &str) -> ProcessStartTrigger {
    ProcessStartTrigger {
        pid: DUMMY_PID,
        ppid: DUMMY_PPID,
        comm: path.split("/").last().unwrap().to_string(),
        argv: cmd.split_whitespace().map(String::from).collect(),
        file_name: path.to_string(),
        started_at: Utc::now(),
    }
}

/// Removes regex characters from a pattern to turn it into a valid path.
/// Currontly only removes leading '^' and strips whitespace.
pub fn pattern_to_path(pattern: &str) -> String {
    let pattern = if pattern[0] == '^' {
        pattern[1..]
    } else {
        pattern
    };
    pattern.trim().to_string()
}
