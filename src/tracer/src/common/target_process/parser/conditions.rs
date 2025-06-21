use crate::common::target_process::target_match::TargetMatch;

#[derive(Clone, Debug)]
pub enum Condition {
    Simple(SimpleCondition),
    And(AndCondition),
    Or(OrCondition),
}

#[derive(Clone, Debug)]
pub struct AndCondition {
    pub and: Vec<Condition>,
}

#[derive(Clone, Debug)]
pub struct OrCondition {
    pub or: Vec<Condition>,
}

// negative rules
//list of command that we want to discard that will be applied to every command
// - process name and command contains
#[derive(Clone, Debug)]
pub enum SimpleCondition {
    ProcessNameIs { process_name_is: String },
    ProcessNameContains { process_name_contains: String },
    MinArgs { min_args: usize },
    ArgsNotContain { args_not_contain: String },
    CommandContains { command_contains: String },
    CommandNotContains { command_not_contains: String },
    CommandMatchesRegex { command_matches_regex: String },
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
            Condition::Simple(SimpleCondition::MinArgs { min_args }) => {
                TargetMatch::MinArgs(*min_args)
            }
            Condition::Simple(SimpleCondition::ArgsNotContain { args_not_contain }) => {
                TargetMatch::ArgsNotContain(args_not_contain.clone())
            }
            Condition::Simple(SimpleCondition::CommandContains { command_contains }) => {
                TargetMatch::CommandContains(command_contains.clone())
            }
            Condition::Simple(SimpleCondition::CommandNotContains {
                command_not_contains,
            }) => TargetMatch::CommandNotContains(command_not_contains.clone()),
            Condition::Simple(SimpleCondition::CommandMatchesRegex {
                command_matches_regex,
            }) => TargetMatch::CommandMatchesRegex(command_matches_regex.clone()),
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
