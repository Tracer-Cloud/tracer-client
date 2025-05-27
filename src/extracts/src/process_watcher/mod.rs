mod utils;
mod watcher;

use std::collections::{HashMap, HashSet};

use tracer_common::{
    target_process::{manager::TargetManager, Target},
    types::trigger::{OomTrigger, ProcessTrigger},
};
pub use watcher::ProcessWatcher;

/// Internal state of the process watcher
struct ProcessState {
    // Maps PIDs to process triggers
    processes: HashMap<usize, ProcessTrigger>,
    // Maps targets to sets of processes being monitored
    monitoring: HashMap<Target, HashSet<ProcessTrigger>>,
    // Groups datasets by the nextflow session UUID
    datasamples_tracker: HashMap<String, HashSet<String>>,
    // List of targets to watch
    target_manager: TargetManager,
    // Store task handle to ensure it stays alive
    ebpf_task: Option<tokio::task::JoinHandle<()>>,

    // tracks relevant processes killed with oom
    oom_victims: HashMap<usize, OomTrigger>, // Map of pid -> oom trigger
}
