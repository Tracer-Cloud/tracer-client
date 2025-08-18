use crate::extracts::process::types::process_state::ProcessState;
use crate::process_identification::utils::log_matched_process;
use std::collections::{HashMap, HashSet};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

/// Filters processes to only include those that match our targets
/// Uses the state's target manager for consistency
pub fn filter_processes_by_target<'a>(
    triggers: &'a [ProcessStartTrigger],
    state: &ProcessState,
) -> HashMap<String, HashSet<&'a ProcessStartTrigger>> {
    triggers
        .iter()
        .flat_map(|trigger| {
            let target = state.get_target_manager().get_target_match(trigger);
            if let Some(matched_target) = target {
                log_matched_process(trigger, &matched_target, true);
                Some((trigger, matched_target))
            } else {
                log_matched_process(trigger, "", false);
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
