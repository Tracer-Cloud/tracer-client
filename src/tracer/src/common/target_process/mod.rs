pub mod target_manager;
pub mod json_rules_parser;

use serde::{Deserialize, Serialize};

/// Simple target matching conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TargetMatch {
    ProcessName(String),
    CommandContains(String),
    Or(Vec<TargetMatch>),
}

/// Display name for targets
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DisplayName {
    Name(String),
}

impl DisplayName {
    pub fn get_display_name(&self) -> String {
        match self {
            DisplayName::Name(name) => name.clone(),
        }
    }
}

/// A target represents a process pattern to match against
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Target {
    pub match_type: TargetMatch,
    pub display_name: DisplayName,
    pub filter_out: Option<Vec<TargetMatch>>,
}

impl Target {
    pub fn new(match_type: TargetMatch) -> Self {
        Self {
            match_type,
            display_name: DisplayName::Name("unknown".to_string()),
            filter_out: None,
        }
    }

    pub fn set_display_name(mut self, display_name: DisplayName) -> Self {
        self.display_name = display_name;
        self
    }

    pub fn set_filter_out(mut self, filter_out: Option<Vec<TargetMatch>>) -> Self {
        self.filter_out = filter_out;
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

        // Check filter_out conditions (all must NOT match)
        if let Some(ref filter_conditions) = self.filter_out {
            for filter_condition in filter_conditions {
                if matches_target(filter_condition, process_name, command) {
                    return false; // Filtered out
                }
            }
        }

        true
    }
}

/// Simple target matching function
pub fn matches_target(target_match: &TargetMatch, process_name: &str, command: &str) -> bool {
    match target_match {
        TargetMatch::ProcessName(name) => process_name == name,
        TargetMatch::CommandContains(content) => command.contains(content),
        TargetMatch::Or(conditions) => {
            conditions.iter().any(|condition| matches_target(condition, process_name, command))
        }
    }
}

/// Command contains structure for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandContainsStruct {
    pub process_name: Option<String>,
    pub command_content: String,
}