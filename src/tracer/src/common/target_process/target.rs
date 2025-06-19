use crate::common::target_process::display_name::DisplayName;
use crate::common::target_process::target_match::{matches_target, TargetMatch};
use serde::{Deserialize, Serialize};

/// A target represents a process pattern to match against
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Target {
    pub match_type: TargetMatch,
    pub display_name: DisplayName,
}

impl Target {
    pub fn new(match_type: TargetMatch) -> Self {
        Self {
            match_type,
            display_name: DisplayName::Name("unknown".to_string()),
        }
    }

    pub fn set_display_name(mut self, display_name: DisplayName) -> Self {
        self.display_name = display_name;
        self
    }

    pub fn get_display_name_object(&self) -> &DisplayName {
        &self.display_name
    }

    pub fn get_display_name_string(&self) -> String {
        self.display_name.get_display_name()
    }

    /// Simple matching logic
    pub fn matches(&self, process_name: &str, command: &str) -> bool {
        // Check if the process matches the primary condition

        if !matches_target(&self.match_type, process_name, command) {
            return false;
        }

        true
    }
}
