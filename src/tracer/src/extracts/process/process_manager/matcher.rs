use crate::extracts::process::types::process_state::ProcessState;
use crate::process_identification::target_process::target::Target;
use crate::process_identification::utils::log_matched_process;
use std::collections::{HashMap, HashSet};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

/// Handles filtering and matching processes against targets
/// Gets targets from the ProcessState instead of holding its own copy
pub struct Filter;

impl Filter {
    /// Finds processes that match our targets
    /// Uses the state's target manager for consistency
    pub fn find_matching_processes<'a>(
        &self,
        triggers: &'a Vec<ProcessStartTrigger>,
        state: &ProcessState,
    ) -> HashMap<String, HashSet<&'a ProcessStartTrigger>> {
        triggers
            .into_iter()
            .flat_map(|trigger| {
                let target = state.get_target_manager().get_target_match(&trigger);
                if let Some(matched_target) = target {
                    log_matched_process(&trigger, &matched_target, true);
                    Some((trigger, matched_target))
                } else {
                    log_matched_process(&trigger, "", false);
                    None
                }
            })
            .fold(
                HashMap::new(),
                |mut matched_processes, (trigger, matched_target)| {
                    matched_processes
                        .entry(matched_target)
                        .or_default()
                        .insert(trigger);
                    matched_processes
                },
            )
    }

    /// Collects all PIDs from the filtered target processes map
    /// TODO: this is never called
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
        Self
    }
}
