use crate::extracts::process::types::process_state::ProcessState;
use crate::extracts::target_process::{Target, TargetMatchable};
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
            if let Some(matched_target) = Self::get_matched_target(state, &trigger) {
                let matched_target = matched_target.clone();
                matched_processes
                    .entry(matched_target)
                    .or_insert(HashSet::new())
                    .insert(trigger);
            }
        }

        Ok(matched_processes)
    }

    /// Gets the matching target for a process, considering both direct matches and parent process matches
    pub fn get_matched_target<'a>(
        state: &'a ProcessState,
        process: &ProcessStartTrigger,
    ) -> Option<&'a Target> {
        // First try direct match
        if let Some(target) = state.get_target_manager().get_target_match(process) {
            return Some(target);
        }

        // If no direct match, try matching through parent processes
        let eligible_targets_for_parents = state
            .get_target_manager()
            .targets
            .iter()
            .filter(|target| !target.should_force_ancestor_to_match())
            .collect::<Vec<_>>();

        if eligible_targets_for_parents.is_empty() {
            return None;
        }

        // Check if any parent process matches an eligible target
        let parents = state.get_process_parents(process);
        for parent in parents {
            for target in eligible_targets_for_parents.iter() {
                if target.matches_process(parent) {
                    return Some(target);
                }
            }
        }

        None
    }

    /// Filters processes to find those of interest based on targets and monitoring state
    ///
    /// This function:
    /// 1. Stores all triggers in the state
    /// 2. Gets PIDs of processes already being monitored
    /// 3. Finds processes that match our targets
    /// 4. Filters out already monitored processes and includes parent processes
    pub async fn filter_processes_of_interest(
        &self,
        triggers: Vec<ProcessStartTrigger>,
        state: &ProcessState,
    ) -> Result<HashMap<Target, HashSet<ProcessStartTrigger>>> {
        // Get PIDs of processes already being monitored
        let already_monitored_pids = state.get_monitored_processes_pids();

        // Find processes that match our targets
        let matched_processes = self.find_matching_processes(triggers, state)?;

        // Filter out already monitored processes and include parent processes
        let interested_in: HashMap<_, _> = matched_processes
            .into_iter()
            .map(|(target, processes)| {
                let processes = processes
                    .into_iter()
                    .flat_map(|proc| {
                        // Get the process and its parents
                        let mut parents = state.get_process_hierarchy(proc);
                        // Filter out already monitored processes
                        parents.retain(|p| !already_monitored_pids.contains(&p.pid));
                        parents
                    })
                    .collect::<HashSet<_>>();

                (target, processes)
            })
            .collect();

        Ok(interested_in)
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
    use crate::extracts::target_process::target_matching::TargetMatch;
    use crate::extracts::target_process::target_process_manager::TargetManager;
    use crate::extracts::target_process::Target;
    use chrono::DateTime;

    #[test]
    fn test_get_matched_target_direct_match() {
        // Create a test target and process
        let target = Target::new(TargetMatch::ProcessName("test_process".to_string()));
        let target_manager = TargetManager::new(vec![target.clone()], vec![]);

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
        let matched_target = Filter::get_matched_target(&state, &process);
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
