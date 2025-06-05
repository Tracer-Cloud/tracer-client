use crate::{
    target_process::{pattern_match, target_matching::TargetMatch, DisplayName},
    types::ebpf_trigger::ProcessStartTrigger,
};

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

    pub fn get_target_match(&self, process: &ProcessStartTrigger) -> Option<Target> {
        let cmd_match = pattern_match::match_process_command(&process.argv.join(" "));
        match cmd_match {
            Ok(_) => {}
            Err(_) => return None,
        };
        let process_name = process.comm.clone();

        let target = Target {
            match_type: TargetMatch::ProcessName(process_name),
            display_name: DisplayName::Default(),
            merge_with_parents: false,
            force_ancestor_to_match: false,
            filter_out: None,
        };

        Some(target)
    }
}
