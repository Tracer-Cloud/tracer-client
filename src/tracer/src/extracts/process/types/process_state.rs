use crate::error_message;
use crate::process_identification::target_pipeline::pipeline_manager::TargetPipelineManager;
use crate::process_identification::target_process::target_manager::TargetManager;
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use tokio::task::JoinHandle;
use tracer_ebpf::ebpf_trigger::{OutOfMemoryTrigger, ProcessStartTrigger};

/// Internal state of the process manager
#[derive(Default)]
pub struct ProcessState {
    processes: HashMap<usize, ProcessStartTrigger>,
    monitoring: HashMap<String, HashSet<ProcessStartTrigger>>,
    target_manager: TargetManager,
    pipeline_manager: TargetPipelineManager,
    ebpf_task: Option<JoinHandle<()>>,
    out_of_memory_victims: HashMap<usize, OutOfMemoryTrigger>,
}

impl ProcessState {
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

    pub fn get_pipeline_manager(&self) -> &TargetPipelineManager {
        &self.pipeline_manager
    }

    pub fn get_pipeline_manager_mut(&mut self) -> &mut TargetPipelineManager {
        &mut self.pipeline_manager
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

    /// Returns the PID of the task that contains the given process.
    ///
    /// Panics if a cycle is detected in the process lineage.
    pub fn get_task_pid(&self, process: &ProcessStartTrigger) -> Option<usize> {
        let mut seen = HashSet::new();
        let mut parent_pid = process.ppid;
        seen.insert(process.pid);
        seen.insert(parent_pid);

        while let Some(parent) = self.get_processes().get(&parent_pid) {
            // TODO: this is nextflow-specific
            if parent.command_string.contains(".command.sh") {
                return Some(parent_pid);
            }

            parent_pid = parent.ppid;

            // Check if we've seen this PID before - that would indicate a cycle
            if seen.contains(&parent_pid) {
                error_message!(
                    "Cycle detected in process lineage! PID {} appears twice in parent chain",
                    parent_pid
                );
                break;
            }

            seen.insert(parent_pid);
        }

        None
    }
}
