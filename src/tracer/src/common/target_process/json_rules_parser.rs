use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use super::{DisplayName, Target, TargetMatch, matches_target};

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
    #[serde(rename = "process_name")]
    ProcessName(String),
    #[serde(rename = "command_contains")]
    CommandContains(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RulesConfig {
    pub rules: Vec<Rule>,
}

impl Condition {
    pub fn to_target_match(&self) -> TargetMatch {
        match self {
            Condition::Simple(SimpleCondition::ProcessName(name)) => {
                TargetMatch::ProcessName(name.clone())
            }
            Condition::Simple(SimpleCondition::CommandContains(content)) => {
                TargetMatch::CommandContains(content.clone())
            }
            Condition::And(and_cond) => {
                // For AND conditions, we need to create a custom TargetMatch that can handle multiple conditions
                // Since TargetMatch doesn't have an And variant, we'll use the first condition as primary
                // and rely on the matches method to handle the AND logic properly
                if let Some(first) = and_cond.and.first() {
                    first.to_target_match()
                } else {
                    // Fallback to a simple condition that never matches
                    TargetMatch::ProcessName("__never_match__".to_string())
                }
            }
            Condition::Or(or_cond) => {
                // For OR conditions, convert all conditions to TargetMatch and use the Or variant
                let target_matches: Vec<TargetMatch> = or_cond.or.iter().map(|condition| condition.to_target_match()).collect();
                TargetMatch::Or(target_matches)
            }
        }
    }
    
    pub fn matches(&self, process_name: &str, command: &str) -> bool {
        match self {
            Condition::Simple(_condition) => {
                let target_match = self.to_target_match();
                matches_target(
                    &target_match,
                    process_name,
                    command,
                )
            }
            Condition::And(and_cond) => {
                and_cond.and.iter().all(|condition| {
                    condition.matches(process_name, command)
                })
            }
            Condition::Or(or_cond) => {
                or_cond.or.iter().any(|condition| {
                    condition.matches(process_name, command)
                })
            }
        }
    }
}

impl Rule {
    pub fn to_target(self) -> Target {
        Target {
            match_type: self.condition.to_target_match(),
            display_name: DisplayName::Name(self.display_name),
            filter_out: None,
        }
    }
}

pub fn load_json_rules<P: AsRef<Path>>(path: P) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    let json_content = fs::read_to_string(path)?;
    let config: RulesConfig = serde_json::from_str(&json_content)?;
    
    let targets: Vec<Target> = config.rules.into_iter().map(|rule| rule.to_target()).collect();
    Ok(targets)
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
        
        let config: RulesConfig = serde_json::from_str(json).unwrap();
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