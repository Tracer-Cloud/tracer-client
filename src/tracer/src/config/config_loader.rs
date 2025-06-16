use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cloud_providers::aws::config::AwsConfig;
use crate::cloud_providers::aws::types::aws_region::AwsRegion;
use crate::common::constants::DEFAULT_DAEMON_PORT;
use crate::common::target_process::targets_list;
use crate::common::target_process::Target;
use crate::constants::{
    AWS_REGION, BATCH_SUBMISSION_INTERVAL_MS, DEFAULT_API_KEY, FILE_SIZE_NOT_CHANGING_PERIOD_MS,
    GRAFANA_WORKSPACE_URL, LOG_FORWARD_ENDPOINT_DEV, LOG_FORWARD_ENDPOINT_PROD, NEW_RUN_PAUSE_MS,
    PROCESS_METRICS_SEND_INTERVAL_MS, PROCESS_POLLING_INTERVAL_MS, SENTRY_DSN,
};
use config::Config as RConfig;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub api_key: String,
    pub process_polling_interval_ms: u64,
    pub batch_submission_interval_ms: u64,
    pub process_metrics_send_interval_ms: u64,
    pub file_size_not_changing_period_ms: u64,
    pub new_run_pause_ms: u64,
    pub targets: Vec<Target>,

    pub aws_init_type: AwsConfig,
    pub aws_region: AwsRegion,

    pub database_secrets_arn: Option<String>,
    pub database_host: Option<String>,
    pub database_name: String,

    pub grafana_workspace_url: String,
    pub server: String,

    pub config_sources: Vec<String>,
    pub sentry_dsn: Option<String>,

    pub log_forward_endpoint_dev: Option<String>,
    pub log_forward_endpoint_prod: Option<String>,
}

pub struct ConfigLoader;

impl ConfigLoader {
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
        }.to_string()
    }
    
    pub fn load_default_config() -> Result<Config> {
        // removing use of toml file
        let mut builder = RConfig::builder();

        // set defaults
        builder = builder
            .set_default("api_key", DEFAULT_API_KEY)?
            .set_default("process_polling_interval_ms", PROCESS_POLLING_INTERVAL_MS)?
            .set_default("batch_submission_interval_ms", BATCH_SUBMISSION_INTERVAL_MS)?
            .set_default("new_run_pause_ms", NEW_RUN_PAUSE_MS)?
            .set_default(
                "file_size_not_changing_period_ms",
                FILE_SIZE_NOT_CHANGING_PERIOD_MS,
            )?
            .set_default(
                "process_metrics_send_interval_ms",
                PROCESS_METRICS_SEND_INTERVAL_MS,
            )?
            .set_default("aws_init_type", AwsConfig::Profile(Self::get_aws_default_profile()))?
            .set_default("aws_region", AWS_REGION)?
            .set_default("database_name", "tracer_db")?
            .set_default("server", format!("127.0.0.1:{}", DEFAULT_DAEMON_PORT))?
            .set_default::<&str, Vec<&str>>("targets", vec![])?
            .set_default("log_forward_endpoint_dev", LOG_FORWARD_ENDPOINT_DEV)?
            .set_default("log_forward_endpoint_prod", LOG_FORWARD_ENDPOINT_PROD)?
            .set_default("sentry_dsn", SENTRY_DSN)?
            .set_default("grafana_workspace_url", GRAFANA_WORKSPACE_URL)?
            .set_default("database_secrets_arn", Some(None::<String>))?
            .set_default("database_host", Some(None::<String>))?;

        // set overrides
        builder = builder.set_override::<&str, Vec<&str>>("config_sources", vec![])?;

        let mut config: Config = builder
            .build()?
            .try_deserialize()
            .context("failed to parse config file")?;

        if config.targets.is_empty() {
            config.targets = targets_list::TARGETS.to_vec()
            // todo: TARGETS shouldn't be specified in the code. Instead, we should have this set in the config file
        }

        Ok(config)
    }
}
