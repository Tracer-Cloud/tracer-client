use crate::common::target_process::parser::conditions::{
    CompoundCondition, Condition, SimpleCondition,
};
use crate::common::target_process::parser::rule::Rule;
use crate::common::target_process::target::Target;
use std::fs;
use std::path::Path;
use yaml_rust2::{Yaml, YamlLoader};

pub fn load_yaml_rules<P: AsRef<Path>>(path: P) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    let path_ref = path.as_ref();
    let yaml_content = fs::read_to_string(path_ref)?;
    load_yaml_rules_from_str(&yaml_content)
}

pub fn load_yaml_rules_from_str(
    yaml_content: &str,
) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    let docs = YamlLoader::load_from_str(yaml_content)?;
    let doc = &docs[0];

    let rules_yaml = &doc["rules"];
    let rules_vec = rules_yaml
        .as_vec()
        .ok_or("Expected 'rules' to be an array")?;

    let rules: Result<Vec<Rule>, _> = rules_vec.iter().map(parse_rule).collect();

    let targets: Vec<Target> = rules?
        .into_iter()
        .map(|rule| rule.clone().into_target())
        .collect();
    Ok(targets)
}

fn parse_rule(yaml: &Yaml) -> Result<Rule, Box<dyn std::error::Error>> {
    let display_name = yaml["display_name"]
        .as_str()
        .ok_or("display_name not found or not a string")?
        .to_string();
    let condition = parse_condition(&yaml["condition"])?;

    Ok(Rule {
        display_name,
        condition,
    })
}

fn parse_condition(yaml: &Yaml) -> Result<Condition, Box<dyn std::error::Error>> {
    if let Some(and_conds) = yaml["and"].as_vec() {
        let conditions = and_conds
            .iter()
            .map(parse_condition)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Condition::And(CompoundCondition(conditions)));
    }

    if let Some(or_conds) = yaml["or"].as_vec() {
        let conditions = or_conds
            .iter()
            .map(parse_condition)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Condition::Or(CompoundCondition(conditions)));
    }

    if let Some(val) = yaml["process_name_is"].as_str() {
        return Ok(Condition::Simple(SimpleCondition::ProcessNameIs {
            process_name_is: val.to_string(),
        }));
    }

    if let Some(val) = yaml["process_name_contains"].as_str() {
        return Ok(Condition::Simple(SimpleCondition::ProcessNameContains {
            process_name_contains: val.to_string(),
        }));
    }

    if let Some(val) = yaml["min_args"].as_i64() {
        return Ok(Condition::Simple(SimpleCondition::MinArgs {
            min_args: val as usize,
        }));
    }

    if let Some(val) = yaml["args_not_contain"].as_str() {
        return Ok(Condition::Simple(SimpleCondition::ArgsNotContain {
            args_not_contain: val.to_string(),
        }));
    }

    if let Some(val) = yaml["first_arg_is"].as_str() {
        return Ok(Condition::Simple(SimpleCondition::FirstArgIs {
            first_arg_is: val.to_string(),
        }));
    }

    if let Some(val) = yaml["command_contains"].as_str() {
        return Ok(Condition::Simple(SimpleCondition::CommandContains {
            command_contains: val.to_string(),
        }));
    }
    if let Some(val) = yaml["command_not_contains"].as_str() {
        return Ok(Condition::Simple(SimpleCondition::CommandNotContains {
            command_not_contains: val.to_string(),
        }));
    }

    if let Some(val) = yaml["command_matches_regex"].as_str() {
        return Ok(Condition::Simple(SimpleCondition::CommandMatchesRegex {
            command_matches_regex: val.to_string(),
        }));
    }

    if let Some(val) = yaml["subcommand_is_one_of"].as_vec() {
        let subcommands = val
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        return Ok(Condition::Simple(SimpleCondition::SubcommandIsOneOf {
            subcommands,
        }));
    }

    Err("Invalid condition".into())
}
