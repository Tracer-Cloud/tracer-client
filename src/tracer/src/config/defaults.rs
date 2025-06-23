use crate::cloud_providers::aws::config::AwsConfig;
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;
use crate::config::Config;
use crate::constants::{
    AWS_REGION, BATCH_SUBMISSION_INTERVAL_MS, DEFAULT_API_KEY, FILE_SIZE_NOT_CHANGING_PERIOD_MS,
    GRAFANA_WORKSPACE_URL, LOG_FORWARD_ENDPOINT_DEV, LOG_FORWARD_ENDPOINT_PROD, NEW_RUN_PAUSE_MS,
    PROCESS_METRICS_SEND_INTERVAL_MS, PROCESS_POLLING_INTERVAL_MS, SENTRY_DSN,
};

fn get_aws_default_profile() -> String {
    match dirs::home_dir() {
        None => "default",
        Some(path) => {
            if std::fs::read_to_string(path.join(".types/credentials"))
                .unwrap_or_default()
                .contains("[me]")
            {
                "me"
            } else {
                "default"
            }
        }
    }
    .to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: DEFAULT_API_KEY.to_string(),
            process_polling_interval_ms: PROCESS_POLLING_INTERVAL_MS,
            batch_submission_interval_ms: BATCH_SUBMISSION_INTERVAL_MS,
            process_metrics_send_interval_ms: PROCESS_METRICS_SEND_INTERVAL_MS,
            file_size_not_changing_period_ms: FILE_SIZE_NOT_CHANGING_PERIOD_MS,
            new_run_pause_ms: NEW_RUN_PAUSE_MS,
            targets: vec![],

            aws_init_type: AwsConfig::Profile(get_aws_default_profile()),
            aws_region: AWS_REGION,

            database_secrets_arn: None,
            database_host: None,
            database_name: "tracer_db".to_string(),

            grafana_workspace_url: GRAFANA_WORKSPACE_URL.to_string(),
            server: format!("127.0.0.1:{}", DEFAULT_DAEMON_PORT),

            config_sources: vec![],
            sentry_dsn: Some(SENTRY_DSN.to_string()),

            log_forward_endpoint_dev: Some(LOG_FORWARD_ENDPOINT_DEV.to_string()),
            log_forward_endpoint_prod: Some(LOG_FORWARD_ENDPOINT_PROD.to_string()),
        }
    }
}
