use crate::process_identification::target_process::parser::yaml_rules_parser::{
    load_yaml_rules, load_yaml_rules_from_str,
};
use crate::process_identification::target_process::target::Target;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::trace;

#[derive(Clone)]
pub struct TargetManager {
    pub exclude: Vec<Target>,
    pub targets: Vec<Target>,
}

impl TargetManager {
    /// Match a process against all targets and return the first matching target name
    pub fn get_target_match(&self, process: &ProcessStartTrigger) -> Option<String> {
        // exclude rules take precedence over rules
        // if one of the exclude rules matches, return None, because we want to exclude the process
        if self
            .exclude
            .iter()
            .any(|target| target.matches(process).is_match)
        {
            return None;
        }

        self.targets.iter().find_map(|target| {
            let process_match = target.matches(process);
            if process_match.is_match {
                let mut display_name = target.get_display_name();

                // Replace {subcommand} if present and subcommand is Some
                if process_match.sub_command.is_some() {
                    let subcommand = process_match.sub_command.unwrap();
                    display_name = display_name.replace("{subcommand}", &subcommand);
                }

                Some(display_name)
            } else {
                None
            }
        })
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
                trace!(
                    "[TargetManager] Failed to load embedded tracer.rules.yml rules: {}",
                    e
                );

                // Fallback to file loading for rules
                let possible_paths_rules = [
                    "process_identification/target_process/yml_rules/tracer.rules.yml",
                    "src/tracer/src/process_identification/target_process/yml_rules/tracer.rules.yml",
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
                            trace!(
                                "[TargetManager] Failed to load YAML rules from {}: {}",
                                rules_path,
                                e
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
                trace!(
                    "[TargetManager] Failed to load embedded tracer.exclude.yml exclude: {}",
                    e
                );

                // Fallback to file loading for exclude
                let possible_paths_exclude = [
                    "process_identification/target_process/yml_rules/tracer.exclude.yml",
                    "src/tracer/src/process_identification/target_process/yml_rules/tracer.exclude.yml",
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
                            trace!(
                                "[TargetManager] Failed to load YAML exclude from {}: {}",
                                exclude_path,
                                e
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
        ProcessStartTrigger::from_name_and_args(0, 0, comm, argv)
    }

    #[test]
    fn test_cat_fastq_target_match() {
        // Load rules from the actual tracer.rules.yml file
        let rules_path = "src/process_identification/target_process/yml_rules/tracer.rules.yml";
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
        assert_eq!(matched.as_deref(), Some("cat FASTQ"));

        let process = make_process("cat", &["cat", "input1/index.1.fastq.gz input.fastq.gz"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("cat FASTQ"));

        // Should NOT match: process_name is 'cat' but command does not contain 'fastq'
        let process = make_process("cat", &["cat"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched, None);

        // Should NOT match: process_name is not 'cat'
        //FIXME
        //let process = make_process("bash", &["cat", "input1/index.1.fastq.gz"]);
        //let matched = manager.get_target_match(&process);
        //assert_eq!(matched, None);
    }

    #[test]
    fn test_exclude_rule() {
        // Load rules from the actual tracer.rules.yml file
        let rules_path = "src/process_identification/target_process/yml_rules/tracer.rules.yml";
        let rules_content =
            fs::read_to_string(rules_path).expect("Failed to read tracer.rules.yml");
        let rules_targets =
            load_yaml_rules_from_str(&rules_content).expect("Failed to parse rules");

        let exclude_path = "src/process_identification/target_process/yml_rules/tracer.exclude.yml";
        let exclude_content =
            fs::read_to_string(exclude_path).expect("Failed to read tracer.exclude.yml");
        let exclude_targets =
            load_yaml_rules_from_str(&exclude_content).expect("Failed to parse exclude");

        let manager = TargetManager {
            targets: rules_targets,
            exclude: exclude_targets,
        };

        let process = make_process("cat", &["cat", "input1/index.1.fastq.gz input.fastq.gz"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("cat FASTQ"));

        // Should NOT match: command contains '--help'
        let process = make_process(
            "cat",
            &["cat", "--help", "input1/index.1.fastq.gz input.fastq.gz"],
        );
        let matched = manager.get_target_match(&process);
        assert_eq!(matched, None);
    }

    #[test]
    fn test_dynamic_display_subcommand() {
        let rules_path = "src/process_identification/target_process/yml_rules/tracer.rules.yml";
        let rules_content =
            fs::read_to_string(rules_path).expect("Failed to read tracer.rules.yml");
        let targets = load_yaml_rules_from_str(&rules_content).expect("Failed to parse rules");

        let target_manager = TargetManager {
            targets,
            exclude: Vec::new(),
        };

        let process = make_process("samtools", &["samtools", "sort", "file.bam"]);
        let matched = target_manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("samtools sort"));

        let process = make_process("samtools", &["samtools", "-@ 4", "sort", "file.bam"]);
        let matched = target_manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("samtools sort"));

        let process = make_process("samtools", &["samtools", "sort -4", "file.bam"]);
        let matched = target_manager.get_target_match(&process);
        assert_eq!(matched, None);
    }
}
