use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsEventType {
    DaemonStartAttempt,
    DaemonStartSuccessful,
    PipelineInitiated,
    // Add more events as needed
}

impl AnalyticsEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnalyticsEventType::DaemonStartAttempt => "daemon_start_attempt",
            AnalyticsEventType::DaemonStartSuccessful => "daemon_start_successful",
            AnalyticsEventType::PipelineInitiated => "pipeline_initiated",
        }
    }
}

#[derive(serde::Serialize)]
pub struct AnalyticsPayload<'a> {
    pub user_id: &'a str,
    pub event_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}
