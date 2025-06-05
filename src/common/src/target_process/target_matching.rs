use serde::{Deserialize, Serialize};

// use std::{borrow::Cow, path::Path};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash, Eq)]
pub struct CommandContainsStruct {
    pub process_name: Option<String>,
    pub command_content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash, Eq)]
pub enum TargetMatch {
    ProcessName(String),
}
