use crate::process_identification::target_process::target_match::{MatchType, ProcessMatch};
use serde::{Deserialize, Serialize};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

/// A target represents a process pattern to match against
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Target {
    match_type: MatchType,
    display_name: String,
}

impl Target {
    pub fn new(match_type: MatchType) -> Self {
        Self {
            match_type,
            display_name: "unknown".to_string(),
        }
    }

    pub fn with_display_name(match_type: MatchType, display_name: String) -> Self {
        Self {
            match_type,
            display_name,
        }
    }

    pub fn match_type_mut(&mut self) -> &mut MatchType {
        &mut self.match_type
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn matches(&self, process: &ProcessStartTrigger) -> bool {
        self.match_type.matches(process)
    }

    pub fn get_match(&self, process: &ProcessStartTrigger) -> Option<String> {
        self.match_type
            .get_match(process)
            .map(|process_match| match process_match {
                ProcessMatch::Simple => self.display_name().to_string(),
                ProcessMatch::Subcommand(sub_command) => {
                    self.display_name().replace("{subcommand}", sub_command)
                }
            })
    }
}
