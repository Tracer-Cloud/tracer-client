use crate::common::target_process::json_rules_parser::load_json_rules;
use crate::common::target_process::Target;
use crate::common::utils::log_matched_process;
use std::path::Path;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

#[derive(Clone)]
pub struct TargetManager {
    pub targets: Vec<Target>,
}

impl TargetManager {
    pub fn new() -> Self {
        // Try multiple possible paths for the rules file
        let possible_paths = vec![
            "common/target_process/default_rules.json",
            "src/tracer/src/common/target_process/default_rules.json",
            "target_process/default_rules.json",
            "default_rules.json",
        ];

        let mut targets = Vec::new();
        let mut loaded = false;

        for rules_path in possible_paths {
            println!("[TargetManager] Trying to load rules from: {}", rules_path);

            match load_json_rules(rules_path) {
                Ok(loaded_targets) => {
                    println!(
                        "[TargetManager] Successfully loaded {} targets from {}",
                        loaded_targets.len(),
                        rules_path
                    );
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

        if !loaded {
            println!("[TargetManager] WARNING: Could not load rules from any path. Using empty target list.");
        }

        Self { targets }
    }

    /// Match a process against all targets and return the first matching target name
    pub fn get_target_match(&self, process: &ProcessStartTrigger) -> Option<String> {
        let command = process.argv.join(" ");

        println!(
            "[TargetManager] Checking process: comm='{}', command='{}'",
            process.comm, command
        );
        println!("[TargetManager] Number of targets: {}", self.targets.len());

        for (i, target) in self.targets.iter().enumerate() {
            println!("[TargetManager] Checking target {}: {:?}", i, target);
            if target.matches(&process.comm, &command) {
                log_matched_process(process, true);
                return Some(target.get_display_name_string());
            } else {
                log_matched_process(process, false);
            }
        }

        None
    }
}
