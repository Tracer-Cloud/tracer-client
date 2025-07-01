use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsEventType {
    DaemonStartAttempted,
    DaemonStartedSuccessfully,
    PipelineInitiated,
}

impl AnalyticsEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnalyticsEventType::DaemonStartAttempted => "daemon_start_attempted",
            AnalyticsEventType::DaemonStartedSuccessfully => "daemon_started_successfully",
            AnalyticsEventType::PipelineInitiated => "pipeline_initiated",
        }
    }
}

#[derive(serde::Serialize)]
pub struct AnalyticsPayload<'a> {
    #[serde(rename = "userId")]
    pub user_id: &'a str,
    pub event_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}
