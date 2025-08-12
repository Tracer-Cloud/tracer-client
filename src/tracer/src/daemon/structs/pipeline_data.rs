use crate::cli::handlers::init_arguments::FinalizedInitArgs;
use crate::daemon::structs::RunSnapshot;
use crate::process_identification::types::pipeline_tags::PipelineTags;
use chrono::{DateTime, TimeDelta, Utc};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PipelineData {
    pub name: String,
    pub start_time: DateTime<Utc>,
    pub is_dev: bool,
    pub tags: PipelineTags,
    pub run_snapshot: Option<RunSnapshot>,
}

impl PipelineData {
    pub fn new(args: &FinalizedInitArgs) -> Self {
        Self {
            name: args.pipeline_name.clone(),
            start_time: Utc::now(),
            is_dev: args.dev,
            tags: args.tags.clone(),
            run_snapshot: None,
        }
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

    pub fn stage(&self) -> &str {
        if self.is_dev {
            "dev"
        } else {
            "prod"
        }
    }
}
