use crate::process_identification::target_process::parser::conditions::{
    CompoundCondition, Condition, SimpleCondition,
};
use crate::process_identification::target_process::parser::rule::Rule;
use crate::utils::yaml::{Yaml, YamlExt, YamlVecLoader};
use anyhow::{anyhow, bail, Result};
use std::path::Path;

pub fn load_yaml_rules<P: AsRef<Path>>(
    embedded_yaml: Option<&str>,
    fallback_paths: &[P],
) -> Vec<Rule> {
    YamlVecLoader {
        module: "TargetManager",
        key: "rules",
        embedded_yaml,
        fallback_paths,
    }
    .load()
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
        ];

        for simple_type in SIMPLE_TYPES {
            if let Some(val) = yaml.optional(simple_type) {
                return match *simple_type {
                    "process_name_is" => Ok(Condition::Simple(SimpleCondition::ProcessNameIs(
                        val.to_string()?,
                    ))),
                    "process_name_contains" => Ok(Condition::Simple(
                        SimpleCondition::ProcessNameContains(val.to_string()?),
                    )),
                    "min_args" => Ok(Condition::Simple(SimpleCondition::MinArgs(val.to_usize()?))),
                    "args_not_contain" => Ok(Condition::Simple(SimpleCondition::ArgsNotContain(
                        val.to_string()?,
                    ))),
                    "first_arg_is" => Ok(Condition::Simple(SimpleCondition::FirstArgIs(
                        val.to_string()?,
                    ))),
                    "command_contains" => Ok(Condition::Simple(SimpleCondition::CommandContains(
                        val.to_string()?,
                    ))),
                    "command_not_contains" => Ok(Condition::Simple(
                        SimpleCondition::CommandNotContains(val.to_string()?),
                    )),
                    "command_matches_regex" => Ok(Condition::Simple(
                        SimpleCondition::CommandMatchesRegex(val.to_string()?),
                    )),
                    "subcommand_is_one_of" => {
                        let subcommands = val
                            .as_vec()
                            .ok_or(anyhow!("Expected an array"))?
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        Ok(Condition::Simple(SimpleCondition::SubcommandIsOneOf(
                            subcommands,
                        )))
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
