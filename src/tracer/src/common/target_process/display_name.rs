use serde::{Deserialize, Serialize};

/// Display name for targets
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DisplayName {
    Name(String),
}

impl DisplayName {
    pub fn get_display_name(&self) -> String {
        match self {
            DisplayName::Name(name) => name.clone(),
        }
    }
}