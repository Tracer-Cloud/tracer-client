// File: src/target/mod.rs
pub mod target_matching;
pub mod targets_list;
use serde::{Deserialize, Serialize};
use target_matching::{matches_target, TargetMatch};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
            DisplayName::Default() => process_name.to_string(),
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
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Target {
    pub match_type: TargetMatch,
    pub display_name: DisplayName,
    pub merge_with_parents: bool,
    pub force_ancestor_to_match: bool,
    pub filter_out: Option<Vec<TargetMatch>>,
}

pub trait TargetMatchable {
    fn matches(&self, process_name: &str, command: &str, bin_path: &str) -> bool;
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

