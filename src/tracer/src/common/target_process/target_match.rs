use regex::Regex;
use serde::{Deserialize, Serialize};

/// Simple target matching conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TargetMatch {
    ProcessNameIs(String),
    ProcessNameContains(String),
    CommandContains(String),
    CommandNotContains(String),
    CommandMatchesRegex(String),
    And(Vec<TargetMatch>),
    Or(Vec<TargetMatch>),
}

/// Simple target matching function
pub fn matches_target(target_match: &TargetMatch, process_name: &str, command: &str) -> bool {
    match target_match {
        TargetMatch::ProcessNameIs(name) => process_name == name,
        TargetMatch::ProcessNameContains(substr) => process_name.contains(substr),
        TargetMatch::CommandContains(content) => command.contains(content),
        TargetMatch::CommandNotContains(content) => !command.contains(content),
        TargetMatch::CommandMatchesRegex(regex_str) => {
            match Regex::new(regex_str) {
                Ok(regex) => regex.is_match(command),
                Err(_) => false, // Invalid regex pattern
            }
        }
        TargetMatch::And(conditions) => conditions
            .iter()
            .all(|condition| matches_target(condition, process_name, command)),
        TargetMatch::Or(conditions) => conditions
            .iter()
            .any(|condition| matches_target(condition, process_name, command)),
    }
}
