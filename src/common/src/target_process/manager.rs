use crate::types::trigger::ProcessTrigger;

use super::Target;

#[derive(Clone, Debug)]
pub struct TargetManager {
    pub targets: Vec<Target>,
    pub blacklist: Vec<Target>,
}

impl TargetManager {
    pub fn new(targets: Vec<Target>, blacklist: Vec<Target>) -> Self {
        Self { targets, blacklist }
    }

    /// Returns None for all processes, effectively capturing everything
    pub fn get_target_match(&self, process: &ProcessTrigger) -> Option<&Target> {
        println!("get_target_match: {:?}", process);
        None
    }
}
