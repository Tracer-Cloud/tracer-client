use std::collections::{HashMap, HashSet};
use tokio::task::JoinHandle;
use tracer_common::target_process::manager::TargetManager;
use tracer_common::target_process::Target;
use tracer_common::types::ebpf_trigger::{OutOfMemoryTrigger, ProcessStartTrigger};

pub enum ProcessResult {
    Found,
    NotFound,
}

/// Internal state of the process manager
pub struct ProcessState {
    processes: HashMap<usize, ProcessStartTrigger>,
    monitoring: HashMap<Target, HashSet<ProcessStartTrigger>>,
    target_manager: TargetManager,
    ebpf_task: Option<JoinHandle<()>>,
    out_of_memory_victims: HashMap<usize, OutOfMemoryTrigger>,
}

impl ProcessState {
    /// Creates a new empty ProcessState
    pub fn new(target_manager: TargetManager) -> Self {
        Self {
            processes: HashMap::new(),
            monitoring: HashMap::new(),
            target_manager,
            ebpf_task: None,
            out_of_memory_victims: HashMap::new(),
        }
    }

    /// Removes a process trigger and returns it if it existed
    pub fn remove_process(&mut self, pid: &usize) -> Option<ProcessStartTrigger> {
        self.processes.remove(pid)
    }

    /// Returns a reference to all processes
    pub fn get_processes(&self) -> &HashMap<usize, ProcessStartTrigger> {
        &self.processes
    }

    // Monitoring related methods

    pub fn get_monitoring(&self) -> HashMap<Target, HashSet<ProcessStartTrigger>> {
        self.monitoring.clone()
    }
    // eBPF task related methods
    /// Sets the eBPF task handle
    pub fn set_ebpf_task(&mut self, task: JoinHandle<()>) {
        self.ebpf_task = Some(task);
    }

    // Out of memory victims related methods

    /// Removes an OOM trigger and returns it if it existed
    pub fn remove_out_of_memory_victim(&mut self, pid: &usize) -> Option<OutOfMemoryTrigger> {
        self.out_of_memory_victims.remove(pid)
    }

    pub fn insert_process(&mut self, pid: usize, process_start_trigger: ProcessStartTrigger) {
        self.processes.insert(pid, process_start_trigger);
    }

    pub fn update_targets(&mut self, targets: Vec<Target>) {
        self.target_manager.targets = targets;
    }

    pub fn insert_out_of_memory_victim(
        &mut self,
        pid: usize,
        out_of_memory_trigger: OutOfMemoryTrigger,
    ) {
        self.out_of_memory_victims
            .insert(pid, out_of_memory_trigger);
    }

    pub fn get_target_manager(&self) -> &TargetManager {
        &self.target_manager
    }
}
