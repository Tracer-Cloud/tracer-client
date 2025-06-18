use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use crate::common::target_process::json_rules_parser::load_json_rules;
use crate::common::target_process::Target;

#[derive(Clone)]
pub struct TargetManager {
    pub targets: Vec<Target>,
}

impl TargetManager {
    pub fn new() -> Self {
        Self { 
            targets: load_json_rules("common/target_process/default_rules.json").unwrap_or_default()
        }
    }

    /// Match a process against all targets and return the first matching target name
    pub fn get_target_match(&self, process: &ProcessStartTrigger) -> Option<String> {
        let command = process.argv.join(" ");
        
        for target in &self.targets {
            if target.matches(&process.comm, &command) {
                return Some(target.get_display_name_string());
            }
        }
        
        None
    }
}