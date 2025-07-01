use crate::process_identification::target_process::target_manager::TargetManager;
use std::collections::{HashMap, HashSet};
use tokio::task::JoinHandle;
use tracer_ebpf::ebpf_trigger::{OutOfMemoryTrigger, ProcessStartTrigger};

/// Internal state of the process manager
pub struct ProcessState {
    processes: HashMap<usize, ProcessStartTrigger>,
    monitoring: HashMap<String, HashSet<ProcessStartTrigger>>,
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_processes(&mut self, processes: HashMap<usize, ProcessStartTrigger>) {
        self.processes = processes;
    }

    // Monitoring related methods

    pub fn get_monitoring(&self) -> &HashMap<String, HashSet<ProcessStartTrigger>> {
        &self.monitoring
    }

    pub fn get_monitoring_mut(&mut self) -> &mut HashMap<String, HashSet<ProcessStartTrigger>> {
        &mut self.monitoring
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

    pub fn update_monitoring(
        &mut self,
        interested_in: HashMap<String, HashSet<ProcessStartTrigger>>,
    ) {
        for (target, processes) in interested_in.into_iter() {
            self.monitoring.entry(target).or_default().extend(processes);
        }
    }

    pub fn get_monitored_processes_pids(&self) -> HashSet<usize> {
        self.monitoring
            .values()
            .flat_map(|processes| processes.iter().map(|p| p.pid))
            .collect()
    }

    /// Gets a process and all its parent processes from the state
    ///
    /// Will panic if a cycle is detected in the process hierarchy.
    pub fn get_process_hierarchy(
        &self,
        process: ProcessStartTrigger,
    ) -> HashSet<ProcessStartTrigger> {
        let mut current_pid = process.ppid;
        let mut hierarchy = HashSet::new();
        // Keep track of visited PIDs to detect cycles
        let mut visited_pids = HashSet::new();

        // Store the process PID before moving the process
        let process_pid = process.pid;

        // Insert the process into the hierarchy (this moves the process)
        hierarchy.insert(process);

        // Add the starting process PID to visited
        visited_pids.insert(process_pid);

        // Traverse up the process tree to include all parent processes
        while let Some(parent) = self.get_processes().get(&current_pid) {
            // Check if we've seen this PID before - that would indicate a cycle
            if visited_pids.contains(&parent.pid) {
                // We have a cycle in the process hierarchy - this shouldn't happen
                // in normal scenarios, but we'll panic to prevent infinite loops
                panic!(
                    "Cycle detected in process hierarchy! PID {} appears twice in parent chain",
                    parent.pid
                );
            }

            // Track that we've visited this PID
            visited_pids.insert(parent.pid);

            // Add parent to the hierarchy
            hierarchy.insert(parent.clone());

            // Move to the next parent
            current_pid = parent.ppid;
        }

        hierarchy
    }
}
