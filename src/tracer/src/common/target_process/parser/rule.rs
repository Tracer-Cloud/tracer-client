use crate::common::target_process::display_name::DisplayName;
use crate::common::target_process::parser::conditions::Condition;
use crate::common::target_process::target::Target;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub rule_name: String,
    pub display_name: String,
    pub condition: Condition,
}

impl Rule {
    pub fn into_target(self) -> Target {
        Target {
            match_type: self.condition.to_target_match(),
            display_name: DisplayName::Name(self.display_name),
        }
    }
}
