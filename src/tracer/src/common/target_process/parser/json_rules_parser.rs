use crate::common::target_process::parser::rules_config::RulesConfig;
use crate::common::target_process::target::Target;
use std::fs;
use std::path::Path;

pub fn load_json_rules<P: AsRef<Path>>(path: P) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    let path_ref = path.as_ref();

    let json_content = fs::read_to_string(path_ref)?;

    load_json_rules_from_str(&json_content)
}

pub fn load_json_rules_from_str(
    json_content: &str,
) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    let config: RulesConfig = serde_json::from_str(json_content)?;

    let targets: Vec<Target> = config
        .rules
        .into_iter()
        .map(|rule| rule.clone().into_target())
        .collect();

    Ok(targets)
}
