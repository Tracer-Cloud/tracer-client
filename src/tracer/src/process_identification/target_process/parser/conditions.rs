use crate::process_identification::target_process::target_match::{CachedRegex, MatchType};
use anyhow::{Error, Result};
use tracing::error;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Condition {
    Simple(SimpleCondition),
    And(CompoundCondition),
    Or(CompoundCondition),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum SimpleCondition {
    /// Matches if the process name is exactly the given string.
    ProcessNameIs { process_name_is: String },
    /// Matches if the process name contains the given substring.
    ProcessNameContains { process_name_contains: String },
    /// Matches if the command has at least the given number of arguments (not including the
    /// process name)
    MinArgs { min_args: usize },
    /// Matches if one of the arguments in the command string (split using shlex) matches the
    /// given string.
    ArgsContain { args_contain: String },
    /// Matches if none of the arguments in the command string (split using shlex) match the
    /// given string.
    ArgsNotContain { args_not_contain: String },
    /// Matches a command of the form `command <arg>`, where `arg` is the given string.
    FirstArgIs { first_arg_is: String },
    /// Matches if the entire command string contains the given substring.
    CommandContains { command_contains: String },
    /// Matches if the command string does not contain the given substring.
    CommandNotContains { command_not_contains: String },
    /// Matches entire command string against a regex.
    CommandMatchesRegex { command_matches_regex: String },
    /// Matches a command of the form `command <subcommand>`, where `subcommand` is one of the
    /// given subcommands.
    SubcommandIsOneOf { subcommands: Vec<String> },
    /// Matches a command of the form `java -jar <jar> [<command>]`.
    Java {
        jar: Option<String>,
        class: Option<String>,
        subcommands: Option<Vec<String>>,
    },
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CompoundCondition(pub Vec<Condition>);

impl CompoundCondition {
    pub fn into_match_types(self) -> Vec<MatchType> {
        self.0
            .into_iter()
            .filter_map(|condition| match condition.try_into() {
                Ok(match_type) => Some(match_type),
                Err(e) => {
                    error!("Error converting condition to match type: {}", e);
                    None
                }
            })
            .collect()
    }
}

impl TryFrom<Condition> for MatchType {
    type Error = Error;

    fn try_from(condition: Condition) -> Result<Self> {
        let match_type = match condition {
            Condition::Simple(SimpleCondition::ProcessNameIs { process_name_is }) => {
                MatchType::ProcessNameIs(process_name_is.clone())
            }
            Condition::Simple(SimpleCondition::ProcessNameContains {
                process_name_contains,
            }) => MatchType::ProcessNameContains(process_name_contains.clone()),
            Condition::Simple(SimpleCondition::MinArgs { min_args }) => {
                MatchType::MinArgs(min_args)
            }
            Condition::Simple(SimpleCondition::ArgsContain { args_contain }) => {
                MatchType::ArgsContain(args_contain.clone())
            }
            Condition::Simple(SimpleCondition::ArgsNotContain { args_not_contain }) => {
                MatchType::ArgsNotContain(args_not_contain.clone())
            }
            Condition::Simple(SimpleCondition::FirstArgIs { first_arg_is }) => {
                MatchType::FirstArgIs(first_arg_is.clone())
            }
            Condition::Simple(SimpleCondition::CommandContains { command_contains }) => {
                MatchType::CommandContains(command_contains.clone())
            }
            Condition::Simple(SimpleCondition::CommandNotContains {
                command_not_contains,
            }) => MatchType::CommandNotContains(command_not_contains.clone()),
            Condition::Simple(SimpleCondition::CommandMatchesRegex {
                command_matches_regex,
            }) => MatchType::CommandMatchesRegex(CachedRegex::new(command_matches_regex)?),
            Condition::Simple(SimpleCondition::SubcommandIsOneOf { subcommands }) => {
                MatchType::SubcommandIsOneOf(subcommands.into())
            }
            Condition::Simple(SimpleCondition::Java {
                jar,
                class,
                subcommands,
            }) => MatchType::Java {
                jar,
                class,
                subcommands: subcommands.map(|cmd| cmd.into()),
            },
            Condition::And(and_condition) => MatchType::And(and_condition.into_match_types()),
            Condition::Or(or_condition) => MatchType::Or(or_condition.into_match_types()),
        };
        Ok(match_type)
    }
}
