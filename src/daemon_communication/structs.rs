use std::collections::HashSet;

use chrono::{DateTime, TimeDelta, Utc};
use itertools::Itertools;

use crate::tracer_client::RunMetadata;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct InfoResponse {
    pub inner: Option<InnerInfoResponse>,
    pub watched_processes_count: usize,
    pub previewed_processes: HashSet<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct InnerInfoResponse {
    pub run_name: String,
    pub run_id: String,
    pub pipeline_name: String,
    pub start_time: DateTime<Utc>,
}

impl InfoResponse {
    pub fn new(
        previewed_processes: HashSet<String>,
        watched_processes_count: usize,
        inner: Option<InnerInfoResponse>,
    ) -> Self {
        Self {
            inner,
            previewed_processes,
            watched_processes_count,
        }
    }
    pub fn watched_processes_preview(&self) -> String {
        self.previewed_processes.iter().join(", ")
    }
}

impl From<RunMetadata> for InnerInfoResponse {
    fn from(value: RunMetadata) -> Self {
        Self {
            run_id: value.id,
            run_name: value.name,
            pipeline_name: value.pipeline_name,
            start_time: value.start_time,
        }
    }
}

impl InnerInfoResponse {
    pub fn total_runtime(&self) -> TimeDelta {
        Utc::now() - self.start_time
    }

    pub fn formatted_runtime(&self) -> String {
        let duration = self.total_runtime();
        format!(
            "{}h {}m {}s",
            duration.num_hours(),
            duration.num_minutes() % 60,
            duration.num_seconds() % 60
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct RunData {
    pub run_name: String,
    pub run_id: String,
    pub pipeline_name: String,
}
