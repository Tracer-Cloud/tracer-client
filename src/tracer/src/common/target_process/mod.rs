pub mod json_rules_parser;
pub mod target_manager;

use serde::{Deserialize, Serialize};

/// Simple target matching conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TargetMatch {
    ProcessNameIs(String),
    ProcessNameContains(String),
    CommandContains(String),
    CommandNotContains(String),
    And(Vec<TargetMatch>),
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

/// Simple target matching function
pub fn matches_target(target_match: &TargetMatch, process_name: &str, command: &str) -> bool {
    match target_match {
        TargetMatch::ProcessNameIs(name) => process_name == name,
        TargetMatch::ProcessNameContains(substr) => process_name.contains(substr),
        TargetMatch::CommandContains(content) => command.contains(content),
        TargetMatch::CommandNotContains(content) => !command.contains(content),
        TargetMatch::And(conditions) => conditions
            .iter()
            .all(|condition| matches_target(condition, process_name, command)),
        TargetMatch::Or(conditions) => conditions
            .iter()
            .any(|condition| matches_target(condition, process_name, command)),
    }
}

/// Command contains structure for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandContainsStruct {
    pub process_name: Option<String>,
    pub command_content: String,
}
