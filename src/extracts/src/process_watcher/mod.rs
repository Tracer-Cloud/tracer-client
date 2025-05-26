mod utils;
mod watcher;

use std::collections::{HashMap, HashSet};

use tracer_common::{
    target_process::{manager::TargetManager, Target},
};
use tracer_common::types::ebpf_trigger::{OutOfMemoryTrigger, ProcessStartTrigger};
pub use watcher::ProcessWatcher;

/// Internal state of the process watcher
struct ProcessState {
    // Maps PIDs to process triggers
    processes: HashMap<usize, ProcessStartTrigger>,
    // Maps targets to sets of processes being monitored
    monitoring: HashMap<Target, HashSet<ProcessStartTrigger>>,
    // Groups datasets by the nextflow session UUID
    datasamples_tracker: HashMap<String, HashSet<String>>,
    // List of targets to watch
    target_manager: TargetManager,
    // Store task handle to ensure it stays alive
    ebpf_task: Option<tokio::task::JoinHandle<()>>,

    // tracks relevant processes killed with oom
    oom_victims: HashMap<usize, OutOfMemoryTrigger>, // Map of pid -> oom trigger
}
