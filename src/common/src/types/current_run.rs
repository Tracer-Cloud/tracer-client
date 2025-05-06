use crate::types::pipeline_tags::PipelineTags;
use chrono::{DateTime, Utc};
use std::time::Instant;

#[derive(Clone)]
pub struct PipelineMetadata {
    pub pipeline_name: String,
    pub run: Option<Run>,
    pub tags: PipelineTags,
}

#[derive(Clone)]
pub struct Run {
    pub name: String,
    pub id: String,
    pub last_interaction: Instant,
    pub start_time: DateTime<Utc>,
    pub parent_pid: Option<usize>,
}

impl Run {
    pub fn new(name: String, id: String) -> Self {
        Run {
            name,
            id,
            last_interaction: Instant::now(),
            start_time: Utc::now(),
            parent_pid: None,
        }
    }
}
