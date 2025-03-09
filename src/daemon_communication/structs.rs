use chrono::{DateTime, TimeDelta, Utc};

use crate::tracer_client::RunMetadata;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct InfoResponse {
    pub run_name: String,
    pub run_id: String,
    pub pipeline_name: String,
    pub start_time: DateTime<Utc>,
}

impl InfoResponse {
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

impl From<RunMetadata> for InfoResponse {
    fn from(value: RunMetadata) -> Self {
        Self {
            run_id: value.id,
            run_name: value.name,
            pipeline_name: value.pipeline_name,
            start_time: value.start_time,
        }
    }
}
