use std::collections::HashSet;

use crate::constants::{GRAFANA_PIPELINE_DASHBOARD_BASE, GRAFANA_RUN_DASHBOARD_BASE};
use crate::process_identification::types::current_run::PipelineMetadata;
use crate::process_identification::types::pipeline_tags::PipelineTags;
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
    pub fn get_pipeline_url(&self) -> String {
        format!(
            "{}?var-pipeline_name={}",
            GRAFANA_PIPELINE_DASHBOARD_BASE, self.pipeline_name
        )
    }
    pub fn get_run_url(&self) -> String {
        format!(
            "{}?var-run_name={}&var-pipeline_name={}",
            GRAFANA_RUN_DASHBOARD_BASE, self.run_name, self.pipeline_name
        )
    }
    pub fn total_runtime(&self) -> TimeDelta {
        Utc::now() - self.start_time
    }

    pub fn formatted_runtime(&self) -> String {
        let duration = self.total_runtime();
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        let seconds = duration.num_seconds() % 60;

        let mut parts = Vec::new();
        if hours > 0 {
            parts.push(format!("{}h", hours));
        }
        if minutes > 0 || hours > 0 {
            parts.push(format!("{}m", minutes));
        }
        parts.push(format!("{}s", seconds));

        parts.join(" ")
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
