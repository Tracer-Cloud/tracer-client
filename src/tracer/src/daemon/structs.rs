use std::collections::HashSet;

use crate::common::types::current_run::PipelineMetadata;
use crate::common::types::pipeline_tags::PipelineTags;
use chrono::{DateTime, TimeDelta, Utc};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

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
    pub tags: PipelineTags,
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

impl TryFrom<PipelineMetadata> for InnerInfoResponse {
    type Error = anyhow::Error;
    fn try_from(value: PipelineMetadata) -> Result<Self, Self::Error> {
        if let Some(run) = value.run {
            Ok(Self {
                run_id: run.id,
                run_name: run.name,
                pipeline_name: value.pipeline_name,
                start_time: run.start_time,
                tags: value.tags,
            })
        } else {
            Err(anyhow::anyhow!("No run found"))
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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TagData {
    pub names: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct LogData {}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub payload: String,
}
