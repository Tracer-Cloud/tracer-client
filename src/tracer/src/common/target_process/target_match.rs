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
/// It returns (bool, Option<String>), the Option<String> is useful when a subcommand is matched to dinamically update the display name
pub fn matches_target(
    target_match: &TargetMatch,
    process: &ProcessStartTrigger,
) -> (bool, Option<String>) {
    match target_match {
        TargetMatch::ProcessNameIs(name) => (process.comm == *name, None),
        TargetMatch::ProcessNameContains(substr) => (process.comm.contains(substr), None),
        TargetMatch::MinArgs(n) => (process.argv.len() > *n, None),
        TargetMatch::ArgsNotContain(content) => {
            (!process.argv.iter().skip(1).any(|arg| arg == content), None)
        }
        TargetMatch::FirstArgIs(arg) => (process.argv.get(1) == Some(arg), None),
        TargetMatch::CommandContains(content) => (process.command_string.contains(content), None),
        TargetMatch::CommandNotContains(content) => {
            (!process.command_string.contains(content), None)
        }
        TargetMatch::CommandMatchesRegex(regex_str) => {
            match Regex::new(regex_str) {
                Ok(regex) => (regex.is_match(&process.command_string), None),
                Err(_) => (false, None), // Invalid regex pattern
            }
        }
        TargetMatch::And(conditions) => {
            // saving the subcommand in case in the AND condition a subcommand is found
            let mut subcommand_found = None;

            let and_conditions_matched = conditions
                .iter()
                .map(|condition| {
                    let (matched, subcommand_name) = matches_target(condition, process);
                    if matched && subcommand_name.is_some() {
                        subcommand_found = Some(subcommand_name.unwrap());
                    }
                    matched
                })
                .all(|matched| matched);

            (and_conditions_matched, subcommand_found)
        }
        TargetMatch::Or(conditions) => {
            // saving the subcommand in case in the OR condition a subcommand is found
            let mut subcommand_found = None;

            let or_conditions_matched = conditions
                .iter()
                .map(|condition| {
                    let (matched, subcommand_name) = matches_target(condition, process);
                    if matched && subcommand_name.is_some() {
                        subcommand_found = Some(subcommand_name.unwrap());
                    }
                    matched
                })
                .any(|matched| matched);

            (or_conditions_matched, subcommand_found)
        }
        TargetMatch::SubcommandIsOneOf(subcommands) => {
            let args = process
                .argv
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();

            // to find the subcommand, we find the first argument that doesn't start with '-' (as options are usually done with -)
            let subcommand = args.iter().skip(1).find(|arg| !arg.starts_with('-'));
            match subcommand {
                Some(cmd) => (subcommands.contains(cmd), Some(cmd.to_string())),
                None => (false, None),
            }
        }
    }
}
