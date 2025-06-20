use crate::common::target_process::parser::json_rules_parser::{
    load_json_rules, load_json_rules_from_str,
};
use crate::common::target_process::target::Target;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

#[derive(Clone)]
pub struct TargetManager {
    pub targets: Vec<Target>,
}

impl TargetManager {

    /// Match a process against all targets and return the first matching target name
    pub fn get_target_match(&self, process: &ProcessStartTrigger) -> Option<String> {
        let command = process.argv.join(" ");

        for target in self.targets.iter() {
            if target.matches(&process.comm, &command) {
                return Some(target.get_display_name_string());
            }
        }

        None
    }
}

impl Default for TargetManager {
    fn default() -> Self {
        // First, try to load from embedded JSON (for production builds)
        match load_json_rules_from_str(include_str!("json_rules/default_rules.json")) {
            Ok(targets) => {
                return Self { targets };
            }
            Err(e) => {
                println!("[TargetManager] Failed to load embedded rules: {}", e);
            }
        }

        // Fallback to file loading (for development)
        let possible_paths = vec![
            "common/target_process/json_rules/default_rules.json",
            "src/tracer/src/common/target_process/json_rules/default_rules.json",
            "target_process/json_rules/default_rules.json",
            "json_rules/default_rules.json",
        ];

        let mut targets = Vec::new();

        for rules_path in possible_paths {
            match load_json_rules(rules_path) {
                Ok(loaded_targets) => {
                    targets = loaded_targets;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

    fn make_process(comm: &str, argv: &[&str]) -> ProcessStartTrigger {
        ProcessStartTrigger {
            pid: 0,
            ppid: 0,
            comm: comm.to_string(),
            file_name: "".to_string(),
            argv: argv.iter().map(|s| s.to_string()).collect(),
            started_at: Default::default(),
        }
    }

    #[test]
    fn test_cat_fastq_target_match() {
        // Load rules from the actual default_rules.json file
        let rules_path = "src/common/target_process/json_rules/default_rules.json";
        let rules_content =
            fs::read_to_string(rules_path).expect("Failed to read default_rules.json");
        let targets = load_json_rules_from_str(&rules_content).expect("Failed to parse rules");
        let manager = TargetManager { targets };

        // Should match: process_name is 'cat' and command contains 'fastq'
        let process = make_process("cat", &["cat", "input1/index.1.fastq.gz"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("CAT FASTQ"));

        // Should NOT match: process_name is 'cat' but command does not contain 'fastq'
        let process = make_process("cat", &["cat"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched, None);

        // Should NOT match: process_name is not 'cat'
        let process = make_process("bash", &["cat", "input1/index.1.fastq.gz"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched, None);
    }
}
