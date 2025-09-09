use crate::constants::{DASHBOARD_BASE_DEV, DASHBOARD_BASE_PROD};
use crate::daemon::structs::OpenTelemetryStatus;
use crate::process_identification::types::current_run::PipelineCostSummary;
use chrono::{DateTime, TimeDelta, Utc};
use itertools::Itertools;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct RunSnapshot {
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) start_time: DateTime<Utc>,
    processes: HashSet<String>,
    tasks: HashMap<String, usize>,
    pub(crate) cost_summary: Option<PipelineCostSummary>,
}

impl RunSnapshot {
    pub fn new(
        name: String,
        id: String,
        processes: HashSet<String>,
        tasks: HashMap<String, usize>,
        cost_summary: Option<PipelineCostSummary>,
        start_time: DateTime<Utc>,
        _opentelemetry_status: Option<OpenTelemetryStatus>,
    ) -> Self {
        Self {
            name,
            id,
            start_time,
            processes,
            tasks,
            cost_summary,
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

    pub fn get_run_url(&self, pipeline_name: String, is_dev: bool) -> String {
        let dashboard_url = if is_dev {
            DASHBOARD_BASE_DEV
        } else {
            DASHBOARD_BASE_PROD
        };

        format!("{}/{}/{}", dashboard_url, pipeline_name, self.id)
    }

    fn total_runtime(&self) -> TimeDelta {
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
