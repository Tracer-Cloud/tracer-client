use crate::config::Config;
use crate::constants::{
    BATCH_SUBMISSION_INTERVAL_MS, BATCH_SUBMISSION_RETRIES,
    BATCH_SUBMISSION_RETRY_DELAY_MS, PROCESS_METRICS_SEND_INTERVAL_MS, PROCESS_POLLING_INTERVAL_MS,
};
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;

impl Default for Config {
    fn default() -> Self {
        Self {
            process_polling_interval_ms: PROCESS_POLLING_INTERVAL_MS,
            batch_submission_interval_ms: BATCH_SUBMISSION_INTERVAL_MS,
            batch_submission_retries: BATCH_SUBMISSION_RETRIES,
            batch_submission_retry_delay_ms: BATCH_SUBMISSION_RETRY_DELAY_MS,
            process_metrics_send_interval_ms: PROCESS_METRICS_SEND_INTERVAL_MS,

            server: format!("127.0.0.1:{}", DEFAULT_DAEMON_PORT),
        }
    }
}
