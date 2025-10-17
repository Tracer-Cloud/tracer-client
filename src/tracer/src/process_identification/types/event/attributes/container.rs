// event/attributes/container.rs
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::extracts::containers::docker_watcher::event::{ContainerEvent, ContainerState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerProperties {
    pub id: String,
    pub name: String,
    pub image: String,
    pub ip: Option<String>,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<chrono::Utc>,
    pub state: ContainerState,
    pub trace_id: Option<String>,
    pub job_id: Option<String>,
    pub env: Vec<String>, // all the environment variables of the container
}

impl From<ContainerEvent> for ContainerProperties {
    fn from(container_event: ContainerEvent) -> Self {
        ContainerProperties {
            id: container_event.id,
            name: container_event.name,
            image: container_event.image,
            ip: container_event.ip,
            labels: container_event.labels,
            timestamp: container_event.timestamp,
            state: container_event.state,
            trace_id: container_event.trace_id,
            job_id: container_event.job_id,
            env: container_event.environment_variables,
        }
    }
}
