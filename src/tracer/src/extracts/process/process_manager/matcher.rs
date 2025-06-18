use crate::common::target_process::Target;
use crate::common::utils::log_matched_process;
use crate::extracts::process::types::process_state::ProcessState;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

/// Handles filtering and matching processes against targets
/// Gets targets from the ProcessState instead of holding its own copy
pub struct Filter;

impl Filter {
    pub fn new() -> Self {
        Self
    }

    /// Finds processes that match our targets
    /// Uses the state's target manager for consistency
    pub fn find_matching_processes(
        &self,
        triggers: Vec<ProcessStartTrigger>,
        state: &ProcessState,
    ) -> Result<HashMap<Target, HashSet<ProcessStartTrigger>>> {
        let mut matched_processes = HashMap::new();

        for trigger in triggers {
            if let Some(matched_target) = state.get_target_manager().get_target_match(&trigger) {
                log_matched_process(&trigger, true);

                let matched_target = matched_target.clone();
                matched_processes
                    .entry(matched_target)
                    .or_insert(HashSet::new())
                    .insert(trigger);
            } else {
                log_matched_process(&trigger, false);
            }
        }

        Ok(matched_processes)
    }

    /// Collects all PIDs from the filtered target processes map
    pub fn collect_pids_to_refresh(
        &self,
        filtered_target_processes: &HashMap<Target, HashSet<ProcessStartTrigger>>,
    ) -> HashSet<usize> {
        filtered_target_processes
            .values()
            .flat_map(|procs| procs.iter().map(|p| p.pid))
            .collect()
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::target_process::target_matching::TargetMatch;
    use crate::common::target_process::target_process_manager::TargetManager;
    use crate::common::target_process::Target;
    use chrono::DateTime;

    #[test]
    fn test_get_matched_target_direct_match() {
        // Create a test target and process
        let target = Target::new(TargetMatch::ProcessName("test_process".to_string()));
        let target_manager = TargetManager::new(vec![target.clone()]);

        let process = create_test_process(
            100,
            1,
            "test_process",
            vec!["test_process", "--arg1", "value1"],
            "/usr/bin/test_process",
        );

        // Create a process state with the target
        let state = ProcessState::new(target_manager);

        // Test direct match
        let matched_target = state.get_target_manager().get_target_match(&process);
        assert!(matched_target.is_some());
        assert_eq!(matched_target.unwrap().match_type, target.match_type);
    }

    // Helper function to create a test process
    fn create_test_process(
        pid: usize,
        ppid: usize,
        comm: &str,
        args: Vec<&str>,
        file_name: &str,
    ) -> ProcessStartTrigger {
        ProcessStartTrigger {
            pid,
            ppid,
            comm: comm.to_string(),
            argv: args.iter().map(|s| s.to_string()).collect(),
            file_name: file_name.to_string(),
            started_at: DateTime::parse_from_rfc3339("2025-05-07T00:00:00Z")
                .unwrap()
                .into(),
        }
    }
}
