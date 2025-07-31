use crate::extracts::process::types::process_state::ProcessState;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio::task::JoinHandle;
use tracer_ebpf::ebpf_trigger::{OutOfMemoryTrigger, ProcessStartTrigger};

/// Manages the process state and provides controlled access to it
#[derive(Default)]
pub struct StateManager {
    state: Arc<RwLock<ProcessState>>,
}

impl StateManager {
    /// Gets a write lock on the process state
    pub async fn get_state_mut(&self) -> RwLockWriteGuard<ProcessState> {
        self.state.write().await
    }

    /// Gets a read lock on the process state
    pub async fn get_state(&self) -> tokio::sync::RwLockReadGuard<ProcessState> {
        self.state.read().await
    }

    /// Sets the eBPF task handle
    pub async fn set_ebpf_task(&self, task: JoinHandle<()>) {
        let mut state = self.get_state_mut().await;
        state.set_ebpf_task(task);
    }

    /// Inserts a process into the state
    pub async fn insert_process(&self, pid: usize, process: ProcessStartTrigger) {
        let mut state = self.state.write().await;
        state.insert_process(pid, process);
    }

    /// Removes a process from the state
    pub async fn remove_process(&self, pid: &usize) {
        let mut state = self.state.write().await;
        state.remove_process(pid);
    }

    /// Inserts an out-of-memory victim
    pub async fn insert_out_of_memory_victim(&self, pid: usize, trigger: OutOfMemoryTrigger) {
        let mut state = self.state.write().await;
        state.insert_out_of_memory_victim(pid, trigger);
    }

    /// Removes an out-of-memory victim
    pub async fn remove_out_of_memory_victim(&self, pid: &usize) -> Option<OutOfMemoryTrigger> {
        let mut state = self.state.write().await;
        state.remove_out_of_memory_victim(pid)
    }

    /// Updates the monitoring state with new processes
    pub async fn update_monitoring(
        &self,
        processes: HashMap<String, HashSet<ProcessStartTrigger>>,
    ) -> Result<()> {
        let mut state = self.state.write().await;
        state.update_monitoring(processes);
        Ok(())
    }

    /// Gets a set of all monitored process names
    pub async fn get_monitored_processes(&self) -> HashSet<String> {
        self.state
            .read()
            .await
            .get_monitoring()
            .keys()
            .cloned()
            .collect()
    }

    /// Gets PIDs of all monitored processes
    pub async fn get_monitored_processes_pids(&self) -> HashSet<usize> {
        let state = self.state.read().await;
        state
            .get_monitoring()
            .iter()
            .flat_map(|(_, processes)| processes.iter().map(|p| p.pid))
            .collect()
    }

    /// Returns a set of matched tasks
    pub async fn get_matched_tasks(&self) -> HashMap<String, usize> {
        let state = self.state.read().await;
        state
            .get_pipeline_manager()
            .matched_tasks()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }
}
