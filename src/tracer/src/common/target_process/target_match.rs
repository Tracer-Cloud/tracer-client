use regex::Regex;
use serde::{Deserialize, Serialize};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

/// Simple target matching conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TargetMatch {
    ProcessNameIs(String),
    ProcessNameContains(String),
    MinArgs(usize),
    ArgsNotContain(String),
    FirstArgIs(String),
    CommandContains(String),
    CommandNotContains(String),
    CommandMatchesRegex(String),
    SubcommandIsOneOf(Vec<String>),
    And(Vec<TargetMatch>),
    Or(Vec<TargetMatch>),
}

/// Simple target matching function
pub fn matches_target(target_match: &TargetMatch, process: &ProcessStartTrigger) -> (bool, Option<String>) {
    match target_match {
        TargetMatch::ProcessNameIs(name) => process.comm == *name,
        TargetMatch::ProcessNameContains(substr) => process.comm.contains(substr),
        TargetMatch::MinArgs(n) => process.argv.len() > *n,
        TargetMatch::ArgsNotContain(content) => {
            !process.argv.iter().skip(1).any(|arg| arg == content)
        }
        TargetMatch::FirstArgIs(arg) => process.argv.get(1) == Some(arg),
        TargetMatch::CommandContains(content) => process.command_string.contains(content),
        TargetMatch::CommandNotContains(content) => !process.command_string.contains(content),
        TargetMatch::CommandMatchesRegex(regex_str) => {
            match Regex::new(regex_str) {
                Ok(regex) => regex.is_match(&process.command_string),
                Err(_) => false, // Invalid regex pattern
            }
        }
        TargetMatch::And(conditions) => conditions
            .iter()
            .all(|condition| matches_target(condition, process)),
        TargetMatch::Or(conditions) => conditions
            .iter()
            .any(|condition| matches_target(condition, process)),
        TargetMatch::SubcommandIsOneOf(subcommands) => {
            let args = process.argv.iter().map(|s| s.to_string()).collect::<Vec<String>>();

            // to find the subcommand, we find the first argument that doesn't start with '-' (as options are usually done with -)
            let subcommand = args.iter().skip(1).find(|arg| !arg.starts_with('-'));
            match subcommand {
                Some(cmd) => subcommands.contains(cmd),
                None => false
            }
        }
    }
}
