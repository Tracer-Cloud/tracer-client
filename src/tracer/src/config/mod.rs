mod defaults;
use crate::cloud_providers::aws::config::AwsConfig;
use crate::cloud_providers::aws::types::aws_region::AwsRegion;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub process_polling_interval_ms: u64,
    pub batch_submission_interval_ms: u64,
    pub batch_submission_retries: u64,
    pub batch_submission_retry_delay_ms: u64,
    pub process_metrics_send_interval_ms: u64,

    pub aws_init_type: AwsConfig,
    pub aws_region: AwsRegion,

    pub database_secrets_arn: Option<String>,
    pub database_host: Option<String>,
    pub database_name: String,

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
            "aws_init_type": self.aws_init_type.to_string(),
            "aws_region": self.aws_region,
            "database_name": self.database_name,
            "server": self.server
        })
    }
}
