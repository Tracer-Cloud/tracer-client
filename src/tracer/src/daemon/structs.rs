use crate::constants::DASHBOARD_BASE;
use crate::process_identification::types::current_run::{PipelineCostSummary, PipelineMetadata};
use crate::process_identification::types::pipeline_tags::PipelineTags;
use chrono::{DateTime, TimeDelta, Utc};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct InfoResponse {
    pub inner: Option<InnerInfoResponse>,
    processes: HashSet<String>,
    tasks: HashMap<String, usize>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct InnerInfoResponse {
    pub run_name: String,
    pub run_id: String,
    pub pipeline_name: String,
    pub start_time: DateTime<Utc>,
    pub tags: PipelineTags,
    pub cost_summary: Option<PipelineCostSummary>,
    pub stage: String,
}

impl InfoResponse {
    pub fn new(
        inner: Option<InnerInfoResponse>,
        processes: HashSet<String>,
        tasks: HashMap<String, usize>,
    ) -> Self {
        Self {
            inner,
            processes,
            tasks,
        }
    }

    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    pub fn processes_preview(&self, limit: Option<(usize, usize)>) -> Vec<String> {
        if let Some((width, items)) = limit {
            let (mut lines, cur_line, _) = self.processes.iter().take(items).fold(
                (Vec::new(), Vec::new(), 0),
                |(mut lines, mut cur_line, mut cur_width), p| {
                    if !cur_line.is_empty() && p.len() > (width.saturating_sub(cur_width + 2)) {
                        lines.push(cur_line.drain(..).join(", "));
                        cur_width = p.len();
                        cur_line.push(p);
                    } else {
                        cur_width += p.len() + 2;
                        cur_line.push(p);
                    }
                    (lines, cur_line, cur_width)
                },
            );
            if !cur_line.is_empty() {
                lines.push(cur_line.into_iter().join(", "));
            }
            lines
        } else {
            vec![self.processes.iter().join(", ")]
        }
    }

    pub fn processes_json(&self) -> Value {
        serde_json::json!(self.processes)
    }

    pub fn tasks_count(&self) -> usize {
        self.tasks.values().sum()
    }

    pub fn tasks_preview(&self, limit: Option<(usize, usize)>) -> Vec<String> {
        let mut task_preview = self.tasks.iter().map(|(task, count)| {
            if *count > 1 {
                format!("{} ({})", task, count)
            } else {
                task.to_owned()
            }
        });
        if let Some((width, items)) = limit {
            let (mut lines, cur_line, _) = task_preview.take(items).fold(
                (Vec::new(), Vec::new(), 0),
                |(mut lines, mut cur_line, mut cur_width), p| {
                    if !cur_line.is_empty() && p.len() > (width.saturating_sub(cur_width + 2)) {
                        lines.push(cur_line.drain(..).join(", "));
                        cur_width = p.len();
                        cur_line.push(p);
                    } else {
                        cur_width += p.len() + 2;
                        cur_line.push(p);
                    }
                    (lines, cur_line, cur_width)
                },
            );
            if !cur_line.is_empty() {
                lines.push(cur_line.into_iter().join(", "));
            }
            lines
        } else {
            vec![task_preview.join(", ")]
        }
    }
}

impl TryFrom<PipelineMetadata> for InnerInfoResponse {
    type Error = anyhow::Error;
    fn try_from(value: PipelineMetadata) -> Result<Self, Self::Error> {
        if let Some(run) = value.run {
            let cost_summary = run
                .cost_summary
                .as_ref()
                .map(|ctx| ctx.refresh(run.start_time));
            let stage = (if value.is_dev { "dev" } else { "prod" }).into();
            Ok(Self {
                run_id: run.id,
                run_name: run.name,
                pipeline_name: value.pipeline_name,
                start_time: run.start_time,
                tags: value.tags,
                cost_summary,
                stage,
            })
        } else {
            Err(anyhow::anyhow!("No run found"))
        }
    }
}

impl InnerInfoResponse {
    pub fn get_run_url(&self) -> String {
        format!(
            "{}/{}/{}",
            DASHBOARD_BASE, self.pipeline_name, self.run_name
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
