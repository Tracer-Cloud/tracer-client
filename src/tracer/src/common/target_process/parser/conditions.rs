use serde::{Deserialize, Serialize};
use crate::common::target_process::target_match::TargetMatch;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "value")]
pub enum Condition {
    #[serde(rename = "simple")]
    Simple(SimpleCondition),
    #[serde(rename = "and")]
    And(AndCondition),
    #[serde(rename = "or")]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "field", content = "value")]
pub enum SimpleCondition {
    #[serde(rename = "process_name_is")]
    ProcessNameIs(String),
    #[serde(rename = "process_name_contains")]
    ProcessNameContains(String),
    #[serde(rename = "command_contains")]
    CommandContains(String),
    #[serde(rename = "command_not_contains")]
    CommandNotContains(String),
}

impl Condition {
    pub fn to_target_match(&self) -> TargetMatch {
        match self {
            Condition::Simple(SimpleCondition::ProcessNameIs(name)) => {
                TargetMatch::ProcessNameIs(name.clone())
            }
            Condition::Simple(SimpleCondition::ProcessNameContains(substr)) => {
                TargetMatch::ProcessNameContains(substr.clone())
            }
            Condition::Simple(SimpleCondition::CommandContains(content)) => {
                TargetMatch::CommandContains(content.clone())
            }
            Condition::Simple(SimpleCondition::CommandNotContains(content)) => {
                TargetMatch::CommandNotContains(content.clone())
            }
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