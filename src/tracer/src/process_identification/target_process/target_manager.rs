use crate::process_identification::target_process::parser::yaml_rules_parser::load_yaml_rules;
use crate::process_identification::target_process::target::Target;
use std::collections::HashSet;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

#[derive(Clone)]
pub struct RuleFile<'a> {
    pub embedded_yaml: Option<&'a str>,
    pub path: Option<&'a str>,
}

#[derive(Clone)]
pub struct TargetManager {
    pub exclude: HashSet<Target>,
    pub targets: HashSet<Target>,
}

impl TargetManager {
    pub fn new<'a>(rule_files: &[RuleFile<'a>], exclude_files: &[RuleFile<'a>]) -> Self {
        let targets = rule_files
            .iter()
            .flat_map(|rf| {
                if let Some(embedded) = rf.embedded_yaml {
                    load_yaml_rules(Some(embedded), &[] as &[&str])
                } else if let Some(path) = rf.path {
                    load_yaml_rules(None, &[std::path::Path::new(path)])
                } else {
                    vec![]
                }
            })
            .map(|rule| rule.into_target())
            .collect::<HashSet<_>>();
        let exclude = exclude_files
            .iter()
            .flat_map(|rf| {
                if let Some(embedded) = rf.embedded_yaml {
                    load_yaml_rules(Some(embedded), &[] as &[&str])
                } else if let Some(path) = rf.path {
                    load_yaml_rules(None, &[std::path::Path::new(path)])
                } else {
                    vec![]
                }
            })
            .map(|rule| rule.into_target())
            .collect::<HashSet<_>>();
        Self { targets, exclude }
    }

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
        // Example: specify embedded files explicitly
        let rule_files = [
            RuleFile {
                embedded_yaml: Some(include_str!("yml_rules/tracer.rules.yml")),
                path: None,
            },
            RuleFile {
                embedded_yaml: Some(include_str!("yml_rules/bioconda.rules.yml")),
                path: None,
            }, // Add more RuleFile entries as needed
        ];
        let exclude_files = [RuleFile {
            embedded_yaml: Some(include_str!("yml_rules/tracer.exclude.yml")),
            path: None,
        }];
        Self::new(&rule_files, &exclude_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

    fn make_process(comm: &str, argv: &[&str]) -> ProcessStartTrigger {
        ProcessStartTrigger::from_name_and_args(0, 0, comm, argv)
    }

    #[test]
    fn test_cat_fastq_target_match() {
        let rule_files = [
            RuleFile {
                embedded_yaml: None,
                path: Some("src/process_identification/target_process/yml_rules/tracer.rules.yml"),
            },
            RuleFile {
                embedded_yaml: None,
                path: Some("src/process_identification/target_process/yml_rules/tracer.rules.yml"),
            },
        ];
        let manager = TargetManager::new(&rule_files, &[]);
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
        let rule_files = [RuleFile {
            embedded_yaml: None,
            path: Some("src/process_identification/target_process/yml_rules/tracer.rules.yml"),
        }];
        let exclude_files = [RuleFile {
            embedded_yaml: None,
            path: Some("src/process_identification/target_process/yml_rules/tracer.exclude.yml"),
        }];
        let manager = TargetManager::new(&rule_files, &exclude_files);
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
        let rule_files = [RuleFile {
            embedded_yaml: None,
            path: Some("src/process_identification/target_process/yml_rules/tracer.rules.yml"),
        }];
        let manager = TargetManager::new(&rule_files, &[]);
        let process = make_process("samtools", &["samtools", "sort", "file.bam"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("samtools sort"));

        let process = make_process("samtools", &["samtools", "-@ 4", "sort", "file.bam"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("samtools sort"));

        let process = make_process("samtools", &["samtools", "sort -4", "file.bam"]);
        let matched = manager.get_target_match(&process);
        assert_eq!(matched, None);
    }
}
