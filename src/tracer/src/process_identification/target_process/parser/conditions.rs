use crate::process_identification::target_process::target_match::{CachedRegex, MatchType};
use anyhow::{Error, Result};
use tracing::error;

#[derive(Clone, Debug)]
pub enum Condition {
    Simple(SimpleCondition),
    And(CompoundCondition),
    Or(CompoundCondition),
}

#[derive(Clone, Debug)]
pub enum SimpleCondition {
    ProcessNameIs { process_name_is: String },
    ProcessNameContains { process_name_contains: String },
    MinArgs { min_args: usize },
    ArgsNotContain { args_not_contain: String },
    FirstArgIs { first_arg_is: String },
    CommandContains { command_contains: String },
    CommandNotContains { command_not_contains: String },
    CommandMatchesRegex { command_matches_regex: String },
    SubcommandIsOneOf { subcommands: Vec<String> },
}

#[derive(Clone, Debug)]
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
            Condition::And(and_condition) => MatchType::And(and_condition.into_match_types()),
            Condition::Or(or_condition) => MatchType::Or(or_condition.into_match_types()),
            Condition::Simple(SimpleCondition::SubcommandIsOneOf { subcommands }) => {
                MatchType::SubcommandIsOneOf(subcommands.into())
            }
        };
        Ok(match_type)
    }
}
