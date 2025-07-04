use crate::process_identification::target_process::target_match::{
    matches_target, MatchType, ProcessMatch,
};
use serde::{Deserialize, Serialize};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

/// A target represents a process pattern to match against
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Target {
    pub match_type: MatchType,
    pub display_name: String,
}

impl Target {
    pub fn new(match_type: MatchType) -> Self {
        Self {
            match_type,
            display_name: "unknown".to_string(),
        }
    }

    pub fn get_display_name(&self) -> String {
        self.display_name.clone()
    }

    /// Simple matching logic
    pub fn matches(&self, process: &ProcessStartTrigger) -> ProcessMatch {
        // Check if the process matches the primary condition
        matches_target(&self.match_type, process)
    }
}
