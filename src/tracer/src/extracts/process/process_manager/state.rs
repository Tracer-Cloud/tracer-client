use crate::common::target_process::target_process_manager::TargetManager;
use crate::common::target_process::Target;
use crate::extracts::process::types::process_state::ProcessState;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio::task::JoinHandle;
use tracer_ebpf::ebpf_trigger::{OutOfMemoryTrigger, ProcessStartTrigger};

/// Manages the process state and provides controlled access to it
pub struct StateManager {
    state: Arc<RwLock<ProcessState>>,
}

impl StateManager {
    pub fn new(target_manager: TargetManager) -> Self {
        Self {
            state: Arc::new(RwLock::new(ProcessState::new(target_manager))),
        }
    }

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

    /// Updates the list of targets being watched
    pub async fn update_targets(&self, targets: Vec<Target>) -> Result<()> {
        let mut state = self.state.write().await;
        state.update_targets(targets);
        Ok(())
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
        processes: HashMap<Target, HashSet<ProcessStartTrigger>>,
    ) -> Result<()> {
        let mut state = self.state.write().await;
        state.update_monitoring(processes);
        Ok(())
    }

    /// Gets the number of monitored processes
    pub async fn get_number_of_monitored_processes(&self) -> usize {
        self.state
            .read()
            .await
            .get_monitoring()
            .values()
            .map(|processes| processes.len())
            .sum()
    }

    /// Gets N process names of monitored processes
    pub async fn get_n_monitored_processes(&self, n: usize) -> HashSet<String> {
        self.state
            .read()
            .await
            .get_monitoring()
            .iter()
            .flat_map(|(_, processes)| processes.iter().map(|p| p.comm.clone()))
            .take(n)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::target_process::target_matching::TargetMatch;
    use crate::common::target_process::Target;
    use chrono::DateTime;

    #[tokio::test]
    async fn test_state_manager_basic_operations() {
        let target = Target::new(TargetMatch::ProcessName("test_process".to_string()));
        let target_manager = TargetManager::new(vec![target]);
        let state_manager = StateManager::new(target_manager);

        // Test initial state
        assert_eq!(state_manager.get_number_of_monitored_processes().await, 0);
        assert!(state_manager
            .get_monitored_processes_pids()
            .await
            .is_empty());

        // Test inserting a process
        let process = create_test_process(100, 1, "test_process");
        state_manager.insert_process(100, process.clone()).await;

        // Verify process was inserted
        let state = state_manager.get_state().await;
        assert!(state.get_processes().contains_key(&100));
    }

    fn create_test_process(pid: usize, ppid: usize, comm: &str) -> ProcessStartTrigger {
        ProcessStartTrigger {
            pid,
            ppid,
            comm: comm.to_string(),
            argv: vec![comm.to_string()],
            file_name: format!("/usr/bin/{}", comm),
            started_at: DateTime::parse_from_rfc3339("2025-05-07T00:00:00Z")
                .unwrap()
                .into(),
        }
    }
}
