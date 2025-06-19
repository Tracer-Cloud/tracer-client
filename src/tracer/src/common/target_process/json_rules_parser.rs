use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use super::{DisplayName, Target, TargetMatch};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub rule_name: String,
    pub display_name: String,
    pub condition: Condition,
}

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RulesConfig {
    pub rules: Vec<Rule>,
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
                if let Some(first) = and_cond.and.first() {
                    first.to_target_match()
                } else {
                    TargetMatch::ProcessNameIs("__never_match__".to_string())
                }
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

    pub fn matches(&self, process_name: &str, command: &str) -> bool {
        match self {
            Condition::Simple(condition) => {
                let result = match condition {
                    SimpleCondition::ProcessNameIs(name) => process_name.eq_ignore_ascii_case(name),
                    SimpleCondition::ProcessNameContains(substr) => {
                        process_name.to_lowercase().contains(&substr.to_lowercase())
                    }
                    SimpleCondition::CommandContains(content) => {
                        command.to_lowercase().contains(&content.to_lowercase())
                    }
                    SimpleCondition::CommandNotContains(content) => {
                        !command.to_lowercase().contains(&content.to_lowercase())
                    }
                };
                println!(
                    "[DEBUG] SimpleCondition: {:?}, process_name: {:?}, command: {:?}, result: {}",
                    condition, process_name, command, result
                );
                result
            }
            Condition::And(and_cond) => {
                let results: Vec<bool> = and_cond.and.iter().map(|condition| {
                    let res = condition.matches(process_name, command);
                    println!("[DEBUG] AndCondition: {:?}, process_name: {:?}, command: {:?}, result: {}", condition, process_name, command, res);
                    res
                }).collect();
                let all = results.iter().all(|&x| x);
                println!(
                    "[DEBUG] AndCondition final: process_name: {:?}, command: {:?}, all: {}",
                    process_name, command, all
                );
                all
            }
            Condition::Or(or_cond) => {
                let results: Vec<bool> = or_cond.or.iter().map(|condition| {
                    let res = condition.matches(process_name, command);
                    println!("[DEBUG] OrCondition: {:?}, process_name: {:?}, command: {:?}, result: {}", condition, process_name, command, res);
                    res
                }).collect();
                let any = results.iter().any(|&x| x);
                println!(
                    "[DEBUG] OrCondition final: process_name: {:?}, command: {:?}, any: {}",
                    process_name, command, any
                );
                any
            }
        }
    }
}

impl Rule {
    pub fn to_target(self) -> Target {
        Target {
            match_type: self.condition.to_target_match(),
            display_name: DisplayName::Name(self.display_name),
        }
    }
}

pub fn load_json_rules<P: AsRef<Path>>(path: P) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    let path_ref = path.as_ref();
    println!("[load_json_rules] Reading file: {:?}", path_ref);

    let json_content = fs::read_to_string(path_ref)?;
    println!(
        "[load_json_rules] File content length: {} bytes",
        json_content.len()
    );

    load_json_rules_from_str(&json_content)
}

pub fn load_json_rules_from_str(
    json_content: &str,
) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    println!(
        "[load_json_rules_from_str] Parsing JSON content of {} bytes",
        json_content.len()
    );

    let config: RulesConfig = serde_json::from_str(json_content)?;
    println!(
        "[load_json_rules_from_str] Parsed {} rules from JSON",
        config.rules.len()
    );

    let targets: Vec<Target> = config
        .rules
        .into_iter()
        .map(|rule| {
            let target = rule.clone().to_target();
            println!(
                "[load_json_rules_from_str] Converted rule '{}' to target: {:?}",
                rule.rule_name.clone(),
                target
            );
            target
        })
        .collect();

    println!(
        "[load_json_rules_from_str] Successfully converted {} targets",
        targets.len()
    );
    Ok(targets)
}

pub fn matches_target(target_match: &TargetMatch, process_name: &str, command: &str) -> bool {
    let result = match target_match {
        TargetMatch::ProcessNameIs(name) => process_name == name,
        TargetMatch::ProcessNameContains(substr) => process_name.contains(substr),
        TargetMatch::CommandContains(content) => command.contains(content),
        TargetMatch::CommandNotContains(content) => !command.contains(content),
        TargetMatch::Or(conditions) => conditions
            .iter()
            .any(|condition| matches_target(condition, process_name, command)),
    };
    println!(
        "[DEBUG] matches_target: {:?}, process_name: {:?}, command: {:?}, result: {}",
        target_match, process_name, command, result
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_condition_parsing() {
        let json = r#"
{
  "rules": [
    {
      "rule_name": "Test Rule",
      "display_name": "test",
      "condition": {
        "type": "simple",
        "value": {
          "field": "process_name_is",
          "value": "test_process"
        }
      }
    }
  ]
}
"#;

        let config: RulesConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].rule_name, "Test Rule");
        assert_eq!(config.rules[0].display_name, "test");
    }

    #[test]
    fn test_and_condition_parsing() {
        let json = r#"
{
  "rules": [
    {
      "rule_name": "Test AND Rule",
      "display_name": "test",
      "condition": {
        "type": "and",
        "value": {
          "and": [
            {
              "type": "simple",
              "value": {
                "field": "process_name_is",
                "value": "perl"
              }
            },
            {
              "type": "simple",
              "value": {
                "field": "command_contains",
                "value": "/opt/conda/bin/fastqc"
              }
            }
          ]
        }
      }
    }
  ]
}
"#;

        let config: RulesConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.rules.len(), 1);

        let rule = &config.rules[0];
        assert!(rule.condition.matches("perl", "/opt/conda/bin/fastqc -t 4"));
        assert!(!rule
            .condition
            .matches("python", "/opt/conda/bin/fastqc -t 4"));
        assert!(!rule.condition.matches("perl", "some other command"));
    }

    #[test]
    fn test_rule_to_target_conversion() {
        let json = r#"
{
  "rules": [
    {
      "rule_name": "Test Rule",
      "display_name": "test_display",
      "condition": {
        "type": "simple",
        "value": {
          "field": "command_contains",
          "value": "test_command"
        }
      }
    }
  ]
}
"#;

        let config: RulesConfig = serde_json::from_str(json).unwrap();
        let target = config.rules[0].clone().to_target();

        assert!(target.matches("any_process", "test_command with args"));
        assert!(!target.matches("any_process", "different command"));
    }

    #[test]
    fn test_cat_fastq_gz_rule() {
        let json = r#"
{
  "rules": [
    {
      "rule_name": "Cat FASTQ",
      "display_name": "CAT FASTQ",
      "condition": {
        "type": "and",
        "value": {
          "and": [
            {
              "type": "simple",
              "value": {
                "field": "process_name_is",
                "value": "cat"
              }
            },
            {
              "type": "simple",
              "value": {
                "field": "command_contains",
                "value": ".fastq.gz"
              }
            }
          ]
        }
      }
    }
  ]
}
"#;
        let config: RulesConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.rules.len(), 1);
        let rule = &config.rules[0];
        // Should match: process name is 'cat' and command contains .fastq.gz
        assert!(rule.condition.matches("cat", "cat input1/index.1.fastq.gz"));
        assert!(rule.condition.matches("cat", "cat foo.fastq.gz bar.txt"));
        // Should NOT match: process name is not 'cat'
        assert!(!rule
            .condition
            .matches("bash", "cat input1/index.1.fastq.gz"));

        // Should NOT match: process name is not 'cat'
        assert!(!rule.condition.matches("cat", "cat"));
        // Should NOT match: command does not contain .fastq.gz
        assert!(!rule.condition.matches("cat", "cat input1/index.1.txt"));
    }

    #[test]
    fn test_command_contains_and_not_contains_case_insensitive() {
        let json = r#"
{
  "rules": [
    {
      "rule_name": "fq command",
      "display_name": "fq",
      "condition": {
        "type": "and",
        "value": {
          "and": [
            {
              "type": "simple",
              "value": {
                "field": "command_contains",
                "value": "fq"
              }
            },
            {
              "type": "simple",
              "value": {
                "field": "command_not_contains",
                "value": "bbsplit"
              }
            }
          ]
        }
      }
    }
  ]
}
"#;
        let config: RulesConfig = serde_json::from_str(json).unwrap();
        let rule = &config.rules[0];
        // Should match: command contains 'fq' but not 'bbsplit'
        assert!(rule
            .condition
            .matches("any_process", "echo fq something else"));
        assert!(rule
            .condition
            .matches("any_process", "echo FQ something else"));
        // Should NOT match: command contains both 'fq' and 'bbsplit' (any case)
        assert!(!rule.condition.matches("any_process", "/opt/conda/bin/bbsplit.sh -Xmx12206M path=bbsplit threads=7 in=WT_REP1_trimmed_1_val_1.fq.gz in2=WT_REP1_trimmed_2_val_2.fq.gz basename=WT_REP1_%_#.fastq.gz refstats=WT_REP1.stats.txt build=1 ambiguous2=all maxindel=150000 ow=f"));
        assert!(!rule.condition.matches("any_process", "echo fq BBSPLIT"));
    }
}
