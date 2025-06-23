use crate::extracts::process::types::process_state::ProcessState;
use crate::process_identification::target_process::target::Target;
use crate::process_identification::utils::log_matched_process;
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
    ) -> HashMap<String, HashSet<ProcessStartTrigger>> {
        triggers
            .into_iter()
            .flat_map(|trigger| {
                let target = state.get_target_manager().get_target_match(&trigger);
                if let Some(rule) = &target {
                    log_matched_process(&trigger, rule, true);
                } else {
                    log_matched_process(&trigger, "", false);
                }
                target.map(|target| (trigger, target))
            })
            .fold(
                HashMap::new(),
                |mut matched_processes, (trigger, matched_target)| {
                    matched_processes
                        .entry(matched_target)
                        .or_insert(HashSet::new())
                        .insert(trigger);
                    matched_processes
                },
            )
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
