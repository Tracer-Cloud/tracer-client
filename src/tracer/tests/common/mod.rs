mod nf_process_match;

pub use nf_process_match::{NextFlowProcessMatcher, ProcessInfo};

use chrono::Utc;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracer_ebpf::ebpf_trigger::Trigger;

pub const DUMMY_PID: usize = 1;
pub const DUMMY_PPID: usize = 0;
pub const PATH_SEPARATOR: &str = "/";

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
