use crate::process_identification::target_process::parser::yaml_rules_parser::load_targets_from_yaml;
use crate::process_identification::target_process::target_set::TargetSet;
use crate::utils::yaml::YamlFile;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

#[derive(Debug, Clone)]
pub struct TargetManager {
    exclude: TargetSet,
    targets: TargetSet,
}

impl TargetManager {
    pub fn new(rule_files: &[YamlFile], exclude_files: &[YamlFile]) -> Self {
        let targets = load_targets_from_yaml(rule_files);
        let exclude = load_targets_from_yaml(exclude_files);
        Self {
            targets: targets.into(),
            exclude: exclude.into(),
        }
    }

    /// Match a process against all targets and return the first matching target name
    pub fn get_target_match(&self, process: &ProcessStartTrigger) -> Option<String> {
        // exclude rules take precedence over rules
        // if one of the exclude rules matches, return None, because we want to exclude the process
        if self.exclude.matches(process) {
            None
        } else {
            self.targets.get_match(process)
        }
    }
}

impl Default for TargetManager {
    fn default() -> Self {
        const RULE_FILES: &[YamlFile] = &[
            YamlFile::Embedded(include_str!("yml_rules/tracer.rules.yml")), // Add more RuleFile entries as needed
        ];
        const EXCLUDE_FILES: &[YamlFile] = &[YamlFile::Embedded(include_str!(
            "yml_rules/tracer.exclude.yml"
        ))];
        Self::new(RULE_FILES, EXCLUDE_FILES)
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
            YamlFile::StaticPath(
                "src/process_identification/target_process/yml_rules/tracer.rules.yml",
            ),
            YamlFile::StaticPath(
                "src/process_identification/target_process/yml_rules/tracer.rules.yml",
            ),
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
    }

    #[test]
    fn test_exclude_rule() {
        let rule_files = [YamlFile::StaticPath(
            "src/process_identification/target_process/yml_rules/tracer.rules.yml",
        )];
        let exclude_files = [YamlFile::StaticPath(
            "src/process_identification/target_process/yml_rules/tracer.exclude.yml",
        )];
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
        let rule_files = [YamlFile::StaticPath(
            "src/process_identification/target_process/yml_rules/tracer.rules.yml",
        )];
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

    #[test]
    fn test_java_command() {
        let rule_files = [YamlFile::StaticPath(
            "src/process_identification/target_process/yml_rules/tracer.rules.yml",
        )];
        let manager = TargetManager::new(&rule_files, &[]);

        let process = make_process(
            "/usr/bin/java",
            &[
                "/usr/bin/java",
                "-Xmx10g",
                "-jar",
                "/foo/bar/fgbio.jar",
                "--async-io=true",
                "ZipperBams",
            ],
        );
        let matched = manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("fgbio ZipperBams"));

        let process = make_process(
            "/usr/bin/java",
            &[
                "/usr/bin/java",
                "-Xmx10g",
                "-jar",
                "/foo/bar/fgbio.jar",
                "--version",
            ],
        );
        let matched = manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), None);

        let process = make_process(
            "java",
            &[
                "java",
                "-Xmx512m",
                "'-Dfastqc.threads=1'",
                "uk.ac.babraham.FastQC.FastQCApplication",
                "CONTROL_REP1_1.gz",
                "CONTROL_REP1_2.gz",
            ],
        );
        let matched = manager.get_target_match(&process);
        assert_eq!(matched.as_deref(), Some("FastQC"));
    }
}
