pub mod attributes;
pub mod aws_metadata;

use attributes::EventAttributes;
use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Event {
    #[serde(with = "ts_seconds")]
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub event_type: String,
    pub process_type: String,
    pub process_status: String,
    pub run_name: Option<String>,
    pub run_id: Option<String>,
    pub attributes: Option<EventAttributes>,
}
