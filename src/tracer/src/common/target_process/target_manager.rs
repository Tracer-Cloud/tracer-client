use crate::common::target_process::parser::yaml_rules_parser::{
    load_yaml_rules, load_yaml_rules_from_str,
};
use crate::common::target_process::target::Target;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use crate::common::target_process::target::Target;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

#[derive(Clone)]
pub struct TargetManager {
    pub exclude: Vec<Target>,
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

        // exclude rules take precedence over rules
        // if one of the exclude rules matches, return None, because we want to exclude the process
        for target in self.exclude.iter() {
            if target.matches(&process.comm, &command) {
                return None;
            }
        }

        // exclude rules take precedence over rules
        // if one of the exclude rules matches, return None, because we want to exclude the process
        for target in self.exclude.iter() {
            if target.matches(&process.comm, &command) {
                return None;
            }
        }

        for target in self.targets.iter() {
            if target.matches(&process.comm, &command) {
                return Some(target.get_display_name());
            }
        }

        None
    }
}

impl Default for TargetManager {
    fn default() -> Self {
        let mut rules_targets = Vec::new();
        let mut exclude_targets = Vec::new();

        // Try to load from embedded YAML (for production builds)

        // Handle individual failures and continue with fallback
        match load_yaml_rules_from_str(include_str!("yml_rules/tracer.rules.yml")) {
            Ok(targets) => {
                rules_targets = targets;
            }
            Err(e) => {
                println!(
                    "[TargetManager] Failed to load embedded tracer.rules.yml rules: {}",
                    e
                );

                // Fallback to file loading for rules
                let possible_paths_rules = [
                    "common/target_process/yml_rules/tracer.rules.yml",
                    "src/tracer/src/common/target_process/yml_rules/tracer.rules.yml",
                    "target_process/yml_rules/tracer.rules.yml",
                    "yml_rules/tracer.rules.yml",
                ];

                for rules_path in possible_paths_rules.iter() {
                    match load_yaml_rules(rules_path) {
                        Ok(loaded_targets) => {
                            rules_targets = loaded_targets;
                            break;
                        }
                        Err(e) => {
                            println!(
                                "[TargetManager] Failed to load YAML rules from {}: {}",
                                rules_path, e
                            );
                        }
                    }
                }
            }
        }

        match load_yaml_rules_from_str(include_str!("yml_rules/tracer.exclude.yml")) {
            Ok(targets) => {
                exclude_targets = targets;
            }
            Err(e) => {
                println!(
                    "[TargetManager] Failed to load embedded tracer.exclude.yml exclude: {}",
                    e
                );

                // Fallback to file loading for exclude
                let possible_paths_exclude = [
                    "common/target_process/yml_rules/tracer.exclude.yml",
                    "src/tracer/src/common/target_process/yml_rules/tracer.exclude.yml",
                    "target_process/yml_rules/tracer.exclude.yml",
                    "yml_rules/tracer.exclude.yml",
                ];

                for exclude_path in possible_paths_exclude.iter() {
                    match load_yaml_rules(exclude_path) {
                        Ok(loaded_targets) => {
                            exclude_targets = loaded_targets;
                            break;
                        }
                        Err(e) => {
                            println!(
                                "[TargetManager] Failed to load YAML exclude from {}: {}",
                                exclude_path, e
                            );
                        }
                    }
                }
            }
        }

        Self {
            exclude: exclude_targets,
            targets: rules_targets,
        }
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
        // Load rules from the actual tracer.rules.yml file
        let rules_path = "src/common/target_process/yml_rules/tracer.rules.yml";
        let rules_content =
            fs::read_to_string(rules_path).expect("Failed to read tracer.rules.yml");
        let targets = load_yaml_rules_from_str(&rules_content).expect("Failed to parse rules");
        let manager = TargetManager {
            targets,
            exclude: Vec::new(),
        };

        // Should match: process_name is 'cat' and command contains 'fastq'
        let process = make_process("cat", &["cat", "input1/index.1.fastq.gz"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched, None);

        let process = make_process("cat", &["cat", "input1/index.1.fastq.gz input.fastq.gz"]);
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

    #[test]
    fn test_exclude_rule() {
        // Load rules from the actual tracer.rules.yml file
        let rules_path = "src/common/target_process/yml_rules/tracer.rules.yml";
        let rules_content =
            fs::read_to_string(rules_path).expect("Failed to read tracer.rules.yml");
        let rules_targets =
            load_yaml_rules_from_str(&rules_content).expect("Failed to parse rules");

        let exclude_path = "src/common/target_process/yml_rules/tracer.exclude.yml";
        let exclude_content =
            fs::read_to_string(exclude_path).expect("Failed to read tracer.exclude.yml");
        let exclude_targets =
            load_yaml_rules_from_str(&exclude_content).expect("Failed to parse exclude");

        let manager = TargetManager {
            targets: rules_targets,
            exclude: exclude_targets,
        };

        let process = make_process(
            "cat",
            &["cat", "--version", "input1/index.1.fastq.gz input.fastq.gz"],
        );
        let matched = manager.get_target_match(&process);
        assert_eq!(matched, None);

        let process = make_process("cat", &["cat", "input1/index.1.fastq.gz input.fastq.gz"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("CAT FASTQ"));

        // Should NOT match: command contains '--help'
        let process = make_process(
            "cat",
            &["cat", "--help", "input1/index.1.fastq.gz input.fastq.gz"],
        );
        let matched = manager.get_target_match(&process);
        assert_eq!(matched, None);
    }
}

impl Default for TargetManager {
    fn default() -> Self {
        Self::new()
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
