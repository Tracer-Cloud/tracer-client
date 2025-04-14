use serde::{Deserialize, Serialize};

use std::{borrow::Cow, path::Path};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CommandContainsStruct {
    pub process_name: Option<String>,
    pub command_content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum TargetMatch {
    ProcessName(String),
    ShortLivedProcessExecutable(String),
    CommandContains(CommandContainsStruct),
    BinPathStartsWith(String),
    BinPathLastComponent(String),
    BinPathContains(String),
}

pub fn to_lowercase(s: &str) -> Cow<str> {
    if s.chars().any(|c| c.is_uppercase()) {
        Cow::Owned(s.to_lowercase())
    } else {
        Cow::Borrowed(s)
    }
}

pub fn process_name_matches(expected_name: &str, process_name: &str) -> bool {
    let process_name_lower = to_lowercase(process_name);
    let expected_name_lower = to_lowercase(expected_name);
    process_name_lower == expected_name_lower
}

pub fn command_contains(command: &str, content: &str) -> bool {
    let command_lower = to_lowercase(command);
    let content_lower = to_lowercase(content);
    command_lower.contains(content_lower.as_ref())
}

pub fn bin_path_starts_with(expected_prefix: &str, bin_path: &str) -> bool {
    let bin_path_lower = to_lowercase(bin_path);
    let expected_prefix_lower = to_lowercase(expected_prefix);
    bin_path_lower.starts_with(expected_prefix_lower.as_ref())
}

pub fn bin_path_last_component_matches(expected_name: &str, bin_path: &str) -> bool {
    let last_component_lower =
        to_lowercase(Path::new(bin_path).file_name().unwrap().to_str().unwrap());
    let name_lower = to_lowercase(expected_name);
    last_component_lower == name_lower
}

pub fn bin_path_contains(expected_content: &str, bin_path: &str) -> bool {
    let bin_path_lower = to_lowercase(bin_path);
    let expected_content_lower = to_lowercase(expected_content);
    bin_path_lower.contains(expected_content_lower.as_ref())
}

pub fn matches_target(
    target: &TargetMatch,
    process_name: &str,
    command: &str,
    bin_path: &str,
) -> bool {
    match target {
        TargetMatch::ProcessName(name) => process_name_matches(name, process_name),
        TargetMatch::BinPathStartsWith(prefix) => bin_path_starts_with(prefix, bin_path),
        TargetMatch::ShortLivedProcessExecutable(name) => command_contains(command, name),
        TargetMatch::CommandContains(inner) => {
            let process_name_matches = inner.process_name.is_none()
                || process_name_matches(inner.process_name.as_ref().unwrap(), process_name);
            process_name_matches && command_contains(command, &inner.command_content)
        }
        TargetMatch::BinPathLastComponent(expected_name) => {
            bin_path_last_component_matches(expected_name, bin_path)
        }
        TargetMatch::BinPathContains(content) => bin_path_contains(content, bin_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target_process::{DisplayName, Target, TargetMatchable};
    use tracer_common::target_process::{DisplayName, Target, TargetMatchable};

    #[test]
    fn test_plotpca_command() {
        let target = Target::new(TargetMatch::CommandContains(CommandContainsStruct {
            process_name: Some("python3.12".to_string()),
            command_content: "plotPCA".to_string(),
        }));
        let process_name = "python3.12";
        let command =
            "/opt/conda/bin/python3.12 /opt/conda/bin/plotPCA -in rnaseq.npz -o PCA_rnaseq_2.png";
        let bin_path = "/opt/conda/bin/python3.12";

        assert!(target.matches(process_name, command, bin_path));

        // Test with incorrect process name
        assert!(!target.matches("python3", command, bin_path));

        // Test with incorrect command content
        assert!(!target.matches(
            process_name,
            "/opt/conda/bin/python3.12 /opt/conda/bin/differentCommand",
            bin_path
        ));

        // Test conda bin with BinPathStarts
        let conda_target =
            Target::new(TargetMatch::BinPathStartsWith("/opt/conda/bin".to_string()));

        assert!(conda_target.matches("any_process", "/opt/conda/bin/somecommand", bin_path));
        assert!(!conda_target.matches(
            "any_process",
            "/usr/bin/somecommand",
            "/usr/bin/somecommand"
        ));
    }

    #[test]
    fn test_plotfingerprint_command() {
        let target = Target::new(TargetMatch::CommandContains(CommandContainsStruct {
            process_name: Some("python3.12".to_string()),
            command_content: "plotFingerprint".to_string(),
        }));
        let process_name = "python3.12";
        let command = "/opt/conda/bin/python3.12 /opt/conda/bin/plotFingerprint -b control.sorted.bam test.sorted.bam --labels Control Test";
        let bin_path = "/opt/conda/bin/python3.12";
        assert!(target.matches(process_name, command, bin_path));

        // Test with incorrect process name
        assert!(!target.matches("python3", command, bin_path));

        // Test with incorrect command content
        assert!(!target.matches(
            process_name,
            "/opt/conda/bin/python3.12 /opt/conda/bin/differentCommand",
            bin_path
        ));
    }

    #[test]
    fn test_kallisto_command() {
        let target = Target::new(TargetMatch::CommandContains(CommandContainsStruct {
            process_name: Some("kallisto".to_string()),
            command_content: "quant".to_string(),
        }));
        let process_name = "kallisto";
        let command =
            "kallisto quant -t 4 -i control_index -o ./control_quant_9 control1_1.fq control1_2.fq";
        let bin_path = "/usr/bin/kallisto";

        assert!(target.matches(process_name, command, bin_path));

        // Test with incorrect process name
        assert!(!target.matches("different_process", command, bin_path));

        // Test with incorrect command content
        assert!(!target.matches(
            process_name,
            "kallisto index -i transcripts.idx transcripts.fa.gz",
            bin_path
        ));
    }

    #[test]
    fn test_conda_bin_always_matches() {
        let target = Target::new(TargetMatch::BinPathStartsWith("/opt/conda/bin".to_string()));
        let process_name = "any_process";
        let command = "/opt/conda/bin/somecommand";
        let bin_path = "/opt/conda/bin/somecommand";

        assert!(target.matches(process_name, command, bin_path));

        // Even with a non-matching process name and different command, it should match due to "/opt/conda/bin"
        assert!(target.matches(
            "different_process",
            "/opt/conda/bin/different_command",
            bin_path
        ));
    }

    #[test]
    fn test_conda_bin_with_specific_process() {
        let target = Target::new(TargetMatch::ProcessName("specific_process".to_string()));
        let command = "/opt/conda/bin/somecommand";
        let bin_path = "/opt/conda/bin/somecommand";

        assert!(target.matches("specific_process", command, bin_path));
        assert!(!target.matches("different_process", command, bin_path));
    }

    #[test]
    fn test_filtering() {
        let target = Target::new(TargetMatch::ProcessName("specific_process".to_string()))
            .set_filter_out(Some(vec![TargetMatch::CommandContains(
                CommandContainsStruct {
                    process_name: Some("specific_process".to_string()),
                    command_content: "filter_me".to_string(),
                },
            )]));

        let command = "/opt/conda/bin/somecommand";
        let bin_path = "/bin/specific_process";

        assert!(target.matches("specific_process", command, bin_path));

        let command = "/opt/conda/bin/somecommand filter_me";
        let bin_path = "/bin/specific_process";

        assert!(!target.matches("specific_process", command, bin_path));
    }

    #[test]
    fn test_multiple_filtering() {
        let target = Target::new(TargetMatch::ProcessName("specific_process".to_string()))
            .set_filter_out(Some(vec![
                TargetMatch::CommandContains(CommandContainsStruct {
                    process_name: None,
                    command_content: "filter_me_one".to_string(),
                }),
                TargetMatch::CommandContains(CommandContainsStruct {
                    process_name: None,
                    command_content: "filter_me_too".to_string(),
                }),
            ]));

        let bin_path = "/bin/specific_process";

        assert!(target.matches("specific_process", "/opt/conda/bin/somecommand", bin_path));

        assert!(!target.matches(
            "specific_process",
            "/opt/conda/bin/somecommand filter_me_one",
            bin_path
        ));

        assert!(!target.matches(
            "specific_process",
            "/opt/conda/bin/somecommand filter_me_too",
            bin_path
        ));

        assert!(target.matches(
            "specific_process",
            "/opt/conda/bin/somecommand filter_me_three",
            bin_path
        ));
    }

    #[test]
    fn test_process_name_case_insensitive() {
        let target = Target::new(TargetMatch::ProcessName("specific_process".to_string()));

        assert!(target.matches(
            "specific_process",
            "/opt/conda/bin/somecommand",
            "/bin/specific_process"
        ));
        assert!(target.matches(
            "Specific_Process",
            "/opt/conda/bin/somecommand",
            "/bin/specific_process"
        ));
        assert!(target.matches(
            "SPECIFIC_PROCESS",
            "/opt/conda/bin/somecommand",
            "/bin/specific_process"
        ));
    }

    #[test]
    fn test_display_name() {
        let target = Target::new(TargetMatch::ProcessName("specific_process".to_string()))
            .set_display_name(DisplayName::Name("Custom Name".to_string()));

        assert_eq!(
            target
                .get_display_name_object()
                .get_display_name("command", &[]),
            "Custom Name"
        );

        let target = Target::new(TargetMatch::ProcessName("specific_process".to_string()))
            .set_display_name(DisplayName::Name("Custom Name".to_string()))
            .set_display_name(DisplayName::Default());

        assert_eq!(
            target
                .get_display_name_object()
                .get_display_name("command", &[]),
            "command"
        );

        let target = Target::new(TargetMatch::ProcessName("specific_process".to_string()))
            .set_display_name(DisplayName::UseFirstArgument());

        assert_eq!(
            target.get_display_name_object().get_display_name(
                "command",
                &[
                    "command".to_string(),
                    "test/test2".to_string(),
                    "arg2".to_string()
                ]
            ),
            "test/test2"
        );

        let target = Target::new(TargetMatch::ProcessName("specific_process".to_string()))
            .set_display_name(DisplayName::UseFirstArgumentBaseName());

        assert_eq!(
            target.get_display_name_object().get_display_name(
                "command",
                &["command".to_string(), "test/test2".to_string()]
            ),
            "test2"
        );
    }
}
