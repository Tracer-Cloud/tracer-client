use crate::common::target_process::parser::rules_config::RulesConfig;
use crate::common::target_process::target::Target;
use serde_yaml;
use std::fs;
use std::path::Path;

pub fn load_yaml_rules<P: AsRef<Path>>(path: P) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    let path_ref = path.as_ref();
    let yaml_content = fs::read_to_string(path_ref)?;
    load_yaml_rules_from_str(&yaml_content)
}

pub fn load_yaml_rules_from_str(
    yaml_content: &str,
) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    let config: RulesConfig = serde_yaml::from_str(yaml_content)?;
    let targets: Vec<Target> = config
        .rules
        .into_iter()
        .map(|rule| rule.clone().into_target())
        .collect();
    Ok(targets)
}
