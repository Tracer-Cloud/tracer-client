use crate::common::target_process::target::Target;
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
    ) -> Result<HashMap<String, HashSet<ProcessStartTrigger>>> {
        let mut matched_processes = HashMap::new();

        for trigger in triggers {
            if let Some(matched_target) = state.get_target_manager().get_target_match(&trigger) {
                log_matched_process(&trigger, &*matched_target, true);

                let matched_target = matched_target.clone();
                matched_processes
                    .entry(matched_target)
                    .or_insert(HashSet::new())
                    .insert(trigger);
            } else {
                log_matched_process(&trigger, "", false);
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
