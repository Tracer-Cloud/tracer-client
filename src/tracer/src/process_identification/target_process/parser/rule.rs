use crate::process_identification::target_process::parser::conditions::Condition;

#[derive(Clone, Debug)]
pub struct Rule {
    pub display_name: String,
    pub condition: Condition,
}
