use crate::common::target_process::json_rules_parser::{load_json_rules, load_json_rules_from_str};
use crate::common::target_process::Target;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

#[derive(Clone)]
pub struct TargetManager {
    pub targets: Vec<Target>,
}

impl TargetManager {
    pub fn new() -> Self {
        // First, try to load from embedded JSON (for production builds)
        match load_json_rules_from_str(include_str!("default_rules.json")) {
            Ok(targets) => {
                return Self { targets };
            }
            Err(e) => {
                println!("[TargetManager] Failed to load embedded rules: {}", e);
            }
        }

        // Fallback to file loading (for development)
        let possible_paths = vec![
            "common/target_process/default_rules.json",
            "src/tracer/src/common/target_process/default_rules.json",
            "target_process/default_rules.json",
            "default_rules.json",
        ];

        let mut targets = Vec::new();
        let mut loaded = false;

        for rules_path in possible_paths {
            match load_json_rules(rules_path) {
                Ok(loaded_targets) => {
                    targets = loaded_targets;
                    loaded = true;
                    break;
                }
                Err(e) => {
                    println!(
                        "[TargetManager] Failed to load rules from {}: {}",
                        rules_path, e
                    );
                }
            }
        }

        Self { targets }
    }

    /// Match a process against all targets and return the first matching target name
    pub fn get_target_match(&self, process: &ProcessStartTrigger) -> Option<String> {
        let command = process.argv.join(" ");

        for (target) in self.targets.iter() {
            if target.matches(&process.comm, &command) {
                return Some(target.get_display_name_string());
            }
        }

        None
    }
}
