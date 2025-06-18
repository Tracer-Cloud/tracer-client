use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::{DisplayName, Target, TargetMatchable};
use crate::common::target_process::target_matching::{CommandContainsStruct, TargetMatch};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub rule_name: String,
    pub display_name: String,
    pub tool_name: Option<String>,
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
    #[serde(rename = "process_name")]
    ProcessName(String),
    #[serde(rename = "command_contains")]
    CommandContains(String),
    #[serde(rename = "bin_path_starts_with")]
    BinPathStartsWith(String),
    #[serde(rename = "bin_path_last_component")]
    BinPathLastComponent(String),
    #[serde(rename = "bin_path_contains")]
    BinPathContains(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct YamlRulesConfig {
    pub rules: Vec<Rule>,
}

impl Condition {
    pub fn to_target_match(&self) -> TargetMatch {
        match self {
            Condition::Simple(SimpleCondition::ProcessName(name)) => {
                TargetMatch::ProcessName(name.clone())
            }
            Condition::Simple(SimpleCondition::CommandContains(content)) => {
                TargetMatch::CommandContains(CommandContainsStruct {
                    process_name: None,
                    command_content: content.clone(),
                })
            }
            Condition::Simple(SimpleCondition::BinPathStartsWith(prefix)) => {
                TargetMatch::BinPathStartsWith(prefix.clone())
            }
            Condition::Simple(SimpleCondition::BinPathLastComponent(name)) => {
                TargetMatch::BinPathLastComponent(name.clone())
            }
            Condition::Simple(SimpleCondition::BinPathContains(content)) => {
                TargetMatch::BinPathContains(content.clone())
            }
            Condition::And(and_cond) => {
                // For AND conditions, we'll use the first condition as the primary match
                if let Some(first) = and_cond.and.first() {
                    first.to_target_match()
                } else {
                    // Fallback to a simple condition that never matches
                    TargetMatch::ProcessName("__never_match__".to_string())
                }
            }
            Condition::Or(or_cond) => {
                // For OR conditions, convert all conditions to TargetMatch and use the Or variant
                let target_matches: Vec<TargetMatch> = or_cond.or.iter().map(|c| c.to_target_match()).collect();
                TargetMatch::Or(target_matches)
            }
        }
    }

    pub fn get_filter_out_conditions(&self) -> Option<Vec<TargetMatch>> {
        match self {
            Condition::And(and_cond) => {
                if and_cond.and.len() > 1 {
                    let filter_conditions: Vec<TargetMatch> = and_cond
                        .and
                        .iter()
                        .skip(1)
                        .map(|c| c.to_target_match())
                        .collect();
                    Some(filter_conditions)
                } else {
                    None
                }
            }
            Condition::Or(_) => None,
            Condition::Simple(_) => None,
        }
    }

    pub fn matches(&self, process_name: &str, command: &str, bin_path: &str) -> bool {
        match self {
            Condition::Simple(_condition) => {
                let target_match = self.to_target_match();
                crate::common::target_process::target_matching::matches_target(
                    &target_match,
                    process_name,
                    command,
                    bin_path,
                )
            }
            Condition::And(and_cond) => {
                and_cond.and.iter().all(|condition| {
                    condition.matches(process_name, command, bin_path)
                })
            }
            Condition::Or(or_cond) => {
                or_cond.or.iter().any(|condition| {
                    condition.matches(process_name, command, bin_path)
                })
            }
        }
    }
}

impl Rule {
    pub fn to_target(&self) -> Target {
        let mut target = Target::new(self.condition.to_target_match())
            .set_display_name(DisplayName::Name(self.display_name.clone()));

        // Add filter_out conditions for AND logic
        if let Some(filter_conditions) = self.condition.get_filter_out_conditions() {
            target = target.set_filter_out(Some(filter_conditions));
        }

        target
    }
}

pub fn load_json_rules<P: AsRef<Path>>(path: P) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: YamlRulesConfig = serde_json::from_str(&content)?;
    
    let targets: Vec<Target> = config.rules.into_iter().map(|rule| rule.to_target()).collect();
    Ok(targets)
}

pub fn load_default_json_rules() -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    // Try to load from the default location
    let default_path = "src/tracer/src/common/target_process/default_rules.json";
    load_json_rules(default_path)
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
          "field": "process_name",
          "value": "test_process"
        }
      }
    }
  ]
}
"#;
        
        let config: YamlRulesConfig = serde_json::from_str(json).unwrap();
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
                "field": "process_name",
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
        
        let config: YamlRulesConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.rules.len(), 1);
        
        let rule = &config.rules[0];
        assert!(rule.condition.matches("perl", "/opt/conda/bin/fastqc -t 4", "/usr/bin/perl"));
        assert!(!rule.condition.matches("python", "/opt/conda/bin/fastqc -t 4", "/usr/bin/python"));
        assert!(!rule.condition.matches("perl", "some other command", "/usr/bin/perl"));
    }

    #[test]
    fn test_or_condition_parsing() {
        let json = r#"
{
  "rules": [
    {
      "rule_name": "Test OR Rule",
      "display_name": "test",
      "condition": {
        "type": "or",
        "value": {
          "or": [
            {
              "type": "simple",
              "value": {
                "field": "bin_path_starts_with",
                "value": "/opt/nextflow/bin"
              }
            },
            {
              "type": "simple",
              "value": {
                "field": "command_contains",
                "value": "nextflow run"
              }
            }
          ]
        }
      }
    }
  ]
}
"#;
        
        let config: YamlRulesConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.rules.len(), 1);
        
        let rule = &config.rules[0];
        assert!(rule.condition.matches("any", "nextflow run pipeline", "/usr/bin/nextflow"));
        assert!(rule.condition.matches("any", "some command", "/opt/nextflow/bin/nextflow"));
        assert!(!rule.condition.matches("any", "some command", "/usr/bin/something"));
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
        
        let config: YamlRulesConfig = serde_json::from_str(json).unwrap();
        let target = config.rules[0].to_target();
        
        assert!(target.matches("any_process", "test_command with args", "/usr/bin/test"));
        assert!(!target.matches("any_process", "different command", "/usr/bin/test"));
    }
} 