pub mod manager;
pub mod pattern_match;
pub mod target_matching;
use crate::types::ebpf_trigger::ProcessStartTrigger;
use serde::{Deserialize, Serialize};
use target_matching::TargetMatch;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash, Eq)]
pub enum DisplayName {
    Default(),
}

impl DisplayName {
    pub fn get_display_name(&self, process_name: &str, commands: &[String]) -> String {
        match self {
            // DisplayName::Name(name) => name.clone(),
            DisplayName::Default() => Self::process_default_display_name(process_name, commands),
        }
    }

    fn process_default_display_name(process_name: &str, commands: &[String]) -> String {
        // First try NextFlow process matching
        if !commands.is_empty() {
            let command_string = commands.join(" ");
            if let Ok(nf_process_name) = pattern_match::match_process_command(&command_string) {
                return nf_process_name;
            }
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
        if let Ok(_) = pattern_match::match_process_command(&command) {
            true
        } else {
            false
        }
    }
}

impl TargetMatchable for Vec<TargetMatch> {
    fn matches(&self, process_name: &str, command: &str, bin_path: &str) -> bool {
        if let Ok(_) = pattern_match::match_process_command(&command) {
            true
        } else {
            false
        }
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
