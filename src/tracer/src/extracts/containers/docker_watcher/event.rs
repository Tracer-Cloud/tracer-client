use chrono::{DateTime, Utc};

use std::collections::HashMap;

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ContainerId(pub String);

impl fmt::Display for ContainerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ContainerState {
    Started,
    Exited { exit_code: i64, reason: String },
    Died,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContainerEvent {
    pub id: String,
    pub name: String,
    pub image: String,
    pub ip: Option<String>,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
    pub state: ContainerState,
    pub environment_variables: Vec<String>,
    pub trace_id: Option<String>,
    pub job_id: Option<String>,
}
