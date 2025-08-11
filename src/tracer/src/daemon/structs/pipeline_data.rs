use crate::cli::handlers::init_arguments::FinalizedInitArgs;
use crate::daemon::structs::RunSnapshot;
use crate::process_identification::types::pipeline_tags::PipelineTags;
use chrono::{DateTime, Utc};

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

    pub fn start_time(&self) -> DateTime<Utc> {
        self.start_time
    }

    pub fn stage(&self) -> &str {
        if self.is_dev {
            "dev"
        } else {
            "prod"
        }
    }
}
