use crate::types::trigger::ProcessTrigger;
// use serde::{Deserialize, Serialize};
// use target_matching::{is_considered_noise, matches_target, TargetMatch};
// use targets_list::DEFAULT_DISPLAY_PROCESS_RULES;

use super::{Target, TargetMatchable};

#[derive(Clone, Debug)]
pub struct TargetManager {
    pub targets: Vec<Target>,
    pub blacklist: Vec<Target>,
}

impl TargetManager {
    pub fn new(targets: Vec<Target>, blacklist: Vec<Target>) -> Self {
        Self { targets, blacklist }
    }

    /// Returns the matching target if it's not blacklisted
    pub fn get_target_match(&self, process: &ProcessTrigger) -> Option<&Target> {
        // Skip blacklisted processes
        if self.blacklist.iter().any(|b| b.matches_process(process)) {
            return None;
        }

        // Return first matching target
        self.targets.iter().find(|t| t.matches_process(process))
    }
}
