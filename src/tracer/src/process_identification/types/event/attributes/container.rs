// event/attributes/container.rs
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::extracts::containers::{ContainerEvent, ContainerState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerProperties {
    pub id: String,
    pub name: String,
    pub image: String,
    pub ip: Option<String>,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<chrono::Utc>,
    pub state: ContainerState,
}

impl From<ContainerEvent> for ContainerProperties {
    fn from(e: ContainerEvent) -> Self {
        ContainerProperties {
            id: e.id,
            name: e.name,
            image: e.image,
            ip: e.ip,
            labels: e.labels,
            timestamp: e.timestamp,
            state: e.state,
        }
    }
}
