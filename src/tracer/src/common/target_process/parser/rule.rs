use crate::common::target_process::parser::conditions::Condition;
use crate::common::target_process::target::Target;

#[derive(Clone, Debug)]
pub struct Rule {
    pub display_name: String,
    pub condition: Condition,
}

impl Rule {
    pub fn into_target(self) -> Target {
        Target {
            match_type: self.condition.to_target_match(),
            display_name: self.display_name,
        }
    }
}
