pub mod defaults;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub process_polling_interval_ms: u64,
    pub batch_submission_interval_ms: u64,
    pub batch_submission_retries: u64,
    pub batch_submission_retry_delay_ms: u64,
    pub process_metrics_send_interval_ms: u64,
    pub server: String,
}

impl Config {
    pub fn to_safe_json(&self) -> Value {
        json!({
            "process_polling_interval_ms": self.process_polling_interval_ms,
            "batch_submission_interval_ms": self.batch_submission_interval_ms,
            "batch_submission_retries": self.batch_submission_retries,
            "batch_submission_retry_delay_ms": self.batch_submission_retry_delay_ms,
            "process_metrics_send_interval_ms": self.process_metrics_send_interval_ms,
            "server": self.server
        })
    }
}
