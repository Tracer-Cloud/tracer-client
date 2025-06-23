use crate::common::target_process::target_match::TargetMatch;

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
    SubcommandIsOneOf { subcommands: Vec<String> }
}

#[derive(Clone, Debug)]
pub struct CompoundCondition(pub Vec<Condition>);

impl CompoundCondition {
    pub fn into_target_matches(self) -> Vec<TargetMatch> {
        self.0
            .into_iter()
            .map(|condition| condition.into_target_match())
            .collect()
    }
}

impl Condition {
    pub fn into_target_match(self) -> TargetMatch {
        match self {
            Condition::Simple(SimpleCondition::ProcessNameIs { process_name_is }) => {
                TargetMatch::ProcessNameIs(process_name_is.clone())
            }
            Condition::Simple(SimpleCondition::ProcessNameContains {
                process_name_contains,
            }) => TargetMatch::ProcessNameContains(process_name_contains.clone()),
            Condition::Simple(SimpleCondition::MinArgs { min_args }) => {
                TargetMatch::MinArgs(min_args)
            }
            Condition::Simple(SimpleCondition::ArgsNotContain { args_not_contain }) => {
                TargetMatch::ArgsNotContain(args_not_contain.clone())
            }
            Condition::Simple(SimpleCondition::FirstArgIs { first_arg_is }) => {
                TargetMatch::FirstArgIs(first_arg_is.clone())
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
            Condition::And(and_condition) => TargetMatch::And(and_condition.into_target_matches()),
            Condition::Or(or_condition) => TargetMatch::Or(or_condition.into_target_matches()),
            Condition::Simple(SimpleCondition::SubcommandIsOneOf { subcommands }) => {
                TargetMatch::SubcommandIsOneOf(subcommands.clone())
            }
        }
    }
}
