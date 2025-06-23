use chrono::{DateTime, Utc};

use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ContainerState {
    Started,
    Exited { exit_code: i64 },
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
}
