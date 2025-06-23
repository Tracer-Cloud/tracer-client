use regex::Regex;
use serde::{Deserialize, Serialize};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

/// Simple target matching conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MatchType {
    ProcessNameIs(String),
    ProcessNameContains(String),
    MinArgs(usize),
    ArgsNotContain(String),
    FirstArgIs(String),
    CommandContains(String),
    CommandNotContains(String),
    CommandMatchesRegex(String),
    SubcommandIsOneOf(Vec<String>),
    And(Vec<MatchType>),
    Or(Vec<MatchType>),
}

pub struct ProcessMatch {
    pub is_match: bool,
    pub sub_command: Option<String>,
}
/// Simple target matching function
/// It returns (bool, Option<String>), the Option<String> is useful when a subcommand is matched to dinamically update the display name
pub fn matches_target(target_match: &MatchType, process: &ProcessStartTrigger) -> ProcessMatch {
    match target_match {
        MatchType::ProcessNameIs(name) => ProcessMatch {
            is_match: process.comm == *name,
            sub_command: None,
        },
        MatchType::ProcessNameContains(substr) => ProcessMatch {
            is_match: process.comm.contains(substr),
            sub_command: None,
        },
        MatchType::MinArgs(n) => ProcessMatch {
            is_match: process.argv.len() > *n,
            sub_command: None,
        },
        MatchType::ArgsNotContain(content) => ProcessMatch {
            is_match: !process.argv.iter().skip(1).any(|arg| arg == content),
            sub_command: None,
        },
        MatchType::FirstArgIs(arg) => ProcessMatch {
            is_match: process.argv.get(1) == Some(arg),
            sub_command: None,
        },
        MatchType::CommandContains(content) => ProcessMatch {
            is_match: process.command_string.contains(content),
            sub_command: None,
        },
        MatchType::CommandNotContains(content) => ProcessMatch {
            is_match: !process.command_string.contains(content),
            sub_command: None,
        },
        MatchType::CommandMatchesRegex(regex_str) => {
            match Regex::new(regex_str) {
                Ok(regex) => ProcessMatch {
                    is_match: regex.is_match(&process.command_string),
                    sub_command: None,
                },
                Err(_) => {
                    // Invalid regex pattern
                    ProcessMatch {
                        is_match: false,
                        sub_command: None,
                    }
                }
            }
        }
        MatchType::And(conditions) => {
            // saving the subcommand in case in the AND condition a subcommand is found
            let mut subcommand_found = None;

            let and_conditions_matched = conditions.iter().all(|condition| {
                let process_match = matches_target(condition, process);
                if process_match.is_match && process_match.sub_command.is_some() {
                    subcommand_found = Some(process_match.sub_command.unwrap());
                }
                process_match.is_match
            });

            ProcessMatch {
                is_match: and_conditions_matched,
                sub_command: subcommand_found,
            }
        }
        MatchType::Or(conditions) => {
            // saving the subcommand in case in the OR condition a subcommand is found
            let mut subcommand_found = None;

            let or_conditions_matched = conditions.iter().any(|condition| {
                let process_match = matches_target(condition, process);
                if process_match.is_match && process_match.sub_command.is_some() {
                    subcommand_found = Some(process_match.sub_command.unwrap());
                }
                process_match.is_match
            });

            ProcessMatch {
                is_match: or_conditions_matched,
                sub_command: subcommand_found,
            }
        }
        MatchType::SubcommandIsOneOf(subcommands) => {
            let args = process
                .argv
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();

            // to find the subcommand, we find the first argument that doesn't start with '-' (as options are usually done with -)
            let subcommand = args.iter().skip(1).find(|arg| !arg.starts_with('-'));

            match subcommand {
                Some(cmd) => ProcessMatch {
                    is_match: subcommands.contains(cmd),
                    sub_command: Some(cmd.to_string()),
                },
                None => ProcessMatch {
                    is_match: false,
                    sub_command: None,
                },
            }
        }
    }
}
