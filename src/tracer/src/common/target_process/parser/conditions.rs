use crate::common::target_process::target_match::TargetMatch;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Condition {
    Simple(SimpleCondition),
    And(AndCondition),
    Or(OrCondition),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AndCondition {
    pub and: Vec<Condition>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OrCondition {
    pub or: Vec<Condition>,
}

// negative rules
//list of command that we want to discard that will be applied to every command
// - process name and command contains
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum SimpleCondition {
    ProcessNameIs { process_name_is: String },
    ProcessNameContains { process_name_contains: String },
    CommandContains { command_contains: String },
    CommandNotContains { command_not_contains: String },
}

impl Condition {
    pub fn to_target_match(&self) -> TargetMatch {
        match self {
            Condition::Simple(SimpleCondition::ProcessNameIs { process_name_is }) => {
                TargetMatch::ProcessNameIs(process_name_is.clone())
            }
            Condition::Simple(SimpleCondition::ProcessNameContains {
                process_name_contains,
            }) => TargetMatch::ProcessNameContains(process_name_contains.clone()),
            Condition::Simple(SimpleCondition::CommandContains { command_contains }) => {
                TargetMatch::CommandContains(command_contains.clone())
            }
            Condition::Simple(SimpleCondition::CommandNotContains {
                command_not_contains,
            }) => TargetMatch::CommandNotContains(command_not_contains.clone()),
            Condition::And(and_cond) => {
                let target_matches: Vec<TargetMatch> = and_cond
                    .and
                    .iter()
                    .map(|condition| condition.to_target_match())
                    .collect();

                TargetMatch::And(target_matches)
            }
            Condition::Or(or_cond) => {
                let target_matches: Vec<TargetMatch> = or_cond
                    .or
                    .iter()
                    .map(|condition| condition.to_target_match())
                    .collect();

                TargetMatch::Or(target_matches)
            }
        }
    }
}
