use serde::{Deserialize, Serialize};
use crate::common::target_process::parser::rule::Rule;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RulesConfig {
    pub rules: Vec<Rule>,
}