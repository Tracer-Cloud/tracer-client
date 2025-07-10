use crate::process_identification::target_process::parser::conditions::{
    CompoundCondition, Condition, SimpleCondition,
};
use crate::process_identification::target_process::parser::rule::Rule;
use crate::process_identification::target_process::target::Target;
use crate::utils::yaml::{self, Yaml, YamlExt, YamlFile};
use anyhow::{anyhow, bail, Result};
use std::collections::HashSet;

pub fn load_targets_from_yaml(yaml_files: &[YamlFile]) -> HashSet<Target> {
    yaml::load_from_yaml_array_files(yaml_files, "rules")
}

impl TryFrom<Yaml> for Target {
    type Error = anyhow::Error;

    fn try_from(yaml: Yaml) -> Result<Self> {
        let rule: Rule = yaml.try_into()?;
        Ok(Target::with_display_name(
            rule.condition.try_into()?,
            rule.display_name,
        ))
    }
}

impl TryFrom<Yaml> for Rule {
    type Error = anyhow::Error;

    fn try_from(yaml: Yaml) -> Result<Self> {
        let display_name = yaml.required_string("display_name")?;
        let condition = yaml.required("condition")?.try_into()?;
        Ok(Rule {
            display_name,
            condition,
        })
    }
}

impl TryFrom<&Yaml> for Condition {
    type Error = anyhow::Error;

    fn try_from(yaml: &Yaml) -> Result<Self> {
        const SIMPLE_TYPES: &[&str] = &[
            "process_name_is",
            "process_name_contains",
            "min_args",
            "args_not_contain",
            "first_arg_is",
            "command_contains",
            "command_not_contains",
            "command_matches_regex",
            "subcommand_is_one_of",
            "java_command",
            "java_command_is_one_of",
        ];

        for simple_type in SIMPLE_TYPES {
            if let Some(val) = yaml.optional(simple_type) {
                return match *simple_type {
                    "process_name_is" => Ok(Condition::Simple(SimpleCondition::ProcessNameIs {
                        process_name_is: val.to_string()?,
                    })),
                    "process_name_contains" => {
                        Ok(Condition::Simple(SimpleCondition::ProcessNameContains {
                            process_name_contains: val.to_string()?,
                        }))
                    }
                    "min_args" => Ok(Condition::Simple(SimpleCondition::MinArgs {
                        min_args: val.to_usize()?,
                    })),
                    "args_not_contain" => Ok(Condition::Simple(SimpleCondition::ArgsNotContain {
                        args_not_contain: val.to_string()?,
                    })),
                    "first_arg_is" => Ok(Condition::Simple(SimpleCondition::FirstArgIs {
                        first_arg_is: val.to_string()?,
                    })),
                    "command_contains" => Ok(Condition::Simple(SimpleCondition::CommandContains {
                        command_contains: val.to_string()?,
                    })),
                    "command_not_contains" => {
                        Ok(Condition::Simple(SimpleCondition::CommandNotContains {
                            command_not_contains: val.to_string()?,
                        }))
                    }
                    "command_matches_regex" => {
                        Ok(Condition::Simple(SimpleCondition::CommandMatchesRegex {
                            command_matches_regex: val.to_string()?,
                        }))
                    }
                    "subcommand_is_one_of" => {
                        let subcommands = val
                            .as_vec()
                            .ok_or(anyhow!("Expected an array"))?
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        Ok(Condition::Simple(SimpleCondition::SubcommandIsOneOf {
                            subcommands,
                        }))
                    }
                    "java_command" => Ok(Condition::Simple(SimpleCondition::JavaCommand(
                        val.to_string()?,
                    ))),
                    "java_command_is_one_of" => {
                        let jar = val.required_string("jar")?;
                        let commands = val
                            .required_vec("commands")?
                            .iter()
                            .map(|command| command.to_string())
                            .collect::<Result<Vec<_>>>()?;
                        Ok(Condition::Simple(SimpleCondition::JavaCommandIsOneOf {
                            jar,
                            commands,
                        }))
                    }
                    _ => bail!("Invalid simple condition type: {}", simple_type),
                };
            }
        }

        const COND_TYPES: &[&str] = &["and", "or"];
        for cond_type in COND_TYPES {
            if let Some(conditions_yml) = yaml.optional_vec(cond_type)? {
                let conditions = CompoundCondition(
                    conditions_yml
                        .iter()
                        .map(|condition| condition.try_into())
                        .collect::<Result<Vec<_>>>()?,
                );
                return match *cond_type {
                    "and" => Ok(Condition::And(conditions)),
                    "or" => Ok(Condition::Or(conditions)),
                    _ => bail!("Unknown condition type: {:?}", cond_type),
                };
            }
        }

        bail!("Invalid step: {:?}", yaml);
    }
}
