// File: src/target/mod.rs
pub mod manager;
pub mod target_matching;
pub mod targets_list;
use crate::types::trigger::ProcessStartTrigger;
use serde::{Deserialize, Serialize};
use target_matching::{matches_target, TargetMatch};
use targets_list::DEFAULT_DISPLAY_PROCESS_RULES;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash, Eq)]
pub enum DisplayName {
    Name(String),
    Default(),
    UseFirstArgument(),
    UseFirstArgumentBaseName(),
}

impl DisplayName {
    pub fn get_display_name(&self, process_name: &str, commands: &[String]) -> String {
        match self {
            DisplayName::Name(name) => name.clone(),
            DisplayName::Default() => Self::process_default_display_name(process_name, commands),
            DisplayName::UseFirstArgument() => commands
                .get(1)
                .unwrap_or(&process_name.to_string())
                .to_string(),
            DisplayName::UseFirstArgumentBaseName() => {
                if commands.is_empty() {
                    return process_name.to_string();
                }
                let first_command = commands
                    .iter()
                    .skip(1)
                    .find(|x| !x.is_empty() && !x.starts_with('-'));
                if first_command.is_none() {
                    return process_name.to_string();
                }
                let base_name = std::path::Path::new(first_command.unwrap()).file_name();
                if base_name.is_none() {
                    return first_command.unwrap().to_string();
                }
                base_name.unwrap().to_str().unwrap().to_string()
            }
        }
    }

    fn process_default_display_name(process_name: &str, commands: &[String]) -> String {
        let tokens: Vec<String> = commands
            .iter()
            .flat_map(|cmd| cmd.split([' ', ';']))
            .map(|token| {
                std::path::Path::new(token)
                    .file_stem()
                    .map(|f| f.to_string_lossy().to_lowercase())
                    .unwrap_or_else(|| token.to_lowercase())
            })
            .collect();

        for label in DEFAULT_DISPLAY_PROCESS_RULES.iter() {
            if tokens.iter().any(|t| t == label) {
                return label.to_string();
            }
        }

        // If process name contains a valid path, return just the file stem
        if let Some(stem) = std::path::Path::new(process_name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
        {
            return stem;
        }

        // Fallback: return as-is
        process_name.to_string()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash, Eq)]
pub struct Target {
    pub match_type: TargetMatch,
    pub display_name: DisplayName,
    pub merge_with_parents: bool,
    pub force_ancestor_to_match: bool,
    pub filter_out: Option<Vec<TargetMatch>>,
}

pub trait TargetMatchable {
    fn matches(&self, process_name: &str, command: &str, bin_path: &str) -> bool;

    fn matches_process(&self, process: &ProcessStartTrigger) -> bool {
        self.matches(
            process.comm.as_str(),
            process.argv.join(" ").as_str(),
            process.file_name.as_str(),
        )
    }
}

impl Target {
    pub fn new(match_type: TargetMatch) -> Target {
        Target {
            match_type,
            display_name: DisplayName::Default(),
            merge_with_parents: true,
            force_ancestor_to_match: true,
            filter_out: None,
        }
    }

    pub fn set_display_name(self, display_name: DisplayName) -> Target {
        Target {
            display_name,
            ..self
        }
    }

    pub fn set_merge_with_parents(self, merge_with_parents: bool) -> Target {
        Target {
            merge_with_parents,
            ..self
        }
    }

    pub fn set_force_ancestor_to_match(self, force_ancestor_to_match: bool) -> Target {
        Target {
            force_ancestor_to_match,
            ..self
        }
    }

    pub fn set_filter_out(self, filter_out: Option<Vec<TargetMatch>>) -> Target {
        Target { filter_out, ..self }
    }

    pub fn should_be_merged_with_parents(&self) -> bool {
        self.merge_with_parents
    }

    pub fn should_force_ancestor_to_match(&self) -> bool {
        self.force_ancestor_to_match
    }

    pub fn get_display_name_object(&self) -> DisplayName {
        self.display_name.clone()
    }
}

impl TargetMatchable for Target {
    fn matches(&self, process_name: &str, command: &str, bin_path: &str) -> bool {
        matches_target(&self.match_type, process_name, command, bin_path)
            && (self.filter_out.is_none()
                || !self
                    .filter_out
                    .as_ref()
                    .unwrap()
                    .matches(process_name, command, bin_path))
    }
}

impl TargetMatchable for Vec<TargetMatch> {
    fn matches(&self, process_name: &str, command: &str, bin_path: &str) -> bool {
        self.iter()
            .any(|target| matches_target(target, process_name, command, bin_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_default_display_name_without_mappings() {
        let commands = vec!["/usr/bin/somebinary".to_string()];
        let process_name = "SomeProcess";

        let display_name = DisplayName::process_default_display_name(process_name, &commands);

        assert_eq!(display_name, "SomeProcess");
    }

    #[test]
    fn test_process_default_display_name_with_perl_wrapped_tool() {
        let commands = vec![
            "perl".to_string(),
            "/opt/conda/bin/fastqc".to_string(),
            "-t".to_string(),
            "7".to_string(),
        ];
        let process_name = "Thread-2";

        let display_name = DisplayName::process_default_display_name(process_name, &commands);

        assert_eq!(display_name, "fastqc");
    }

    #[test]
    fn test_process_default_display_name_with_bash_wrapped_script() {
        let commands = vec![
            "/bin/bash".to_string(),
            "/opt/conda/bin/bbsplit.sh".to_string(),
            "in=sample.fq.gz".to_string(),
        ];
        let process_name = "Thread-9";

        let display_name = DisplayName::process_default_display_name(process_name, &commands);

        assert_eq!(display_name, "bbsplit");
    }

    #[test]
    fn test_process_default_display_name_with_semicolon_chaining() {
        let commands = vec![
            "bash".to_string(),
            "-c".to_string(),
            ". spack/share/spack/setup-env.sh; fastqc sample.fq.gz".to_string(),
        ];
        let process_name = "Thread-10";

        let display_name = DisplayName::process_default_display_name(process_name, &commands);

        assert_eq!(display_name, "fastqc");
    }

    #[test]
    fn test_process_default_display_name_with_non_matching_tokens() {
        let commands = vec![
            "bash".to_string(),
            "-c".to_string(),
            "echo hello world".to_string(),
        ];
        let process_name = "Thread-11";

        let display_name = DisplayName::process_default_display_name(process_name, &commands);

        assert_eq!(display_name, "Thread-11");
    }

    #[test]
    fn test_process_default_display_name_bgzip() {
        let commands = vec![
            "bgzip".to_string(),
            "-c".to_string(),
            "-f".to_string(),
            "-l".to_string(),
            "4".to_string(),
            "@".to_string(),
            "7".to_string(),
        ];
        let process_name = "/opt/conda/bin/bgzip";

        let display_name = DisplayName::process_default_display_name(process_name, &commands);

        assert_eq!(display_name, "bgzip");
    }
}
