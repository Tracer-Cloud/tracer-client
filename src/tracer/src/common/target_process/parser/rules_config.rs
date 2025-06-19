use crate::common::target_process::parser::rule::Rule;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RulesConfig {
    pub rules: Vec<Rule>,
}
