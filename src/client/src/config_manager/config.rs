use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use log::info;
use serde::{Deserialize, Serialize};

use crate::config_manager::bashrc_intercept::{
    modify_bashrc_file, rewrite_interceptor_bashrc_file,
};

use config::{Case, Config as RConfig, Environment, File};
use tracer_aws::config::AwsConfig;
use tracer_aws::types::aws_region::AwsRegion;
use tracer_common::target_process::target_matching::TargetMatch;
use tracer_common::target_process::targets_list;
use tracer_common::target_process::Target;

const DEFAULT_API_KEY: &str = "EAjg7eHtsGnP3fTURcPz1";
const DEFAULT_CONFIG_FILE_LOCATION_FROM_HOME: &str = ".config/tracer";
const PROCESS_POLLING_INTERVAL_MS: u64 = 5;
const BATCH_SUBMISSION_INTERVAL_MS: u64 = 5000;
const NEW_RUN_PAUSE_MS: u64 = 10 * 60 * 1000;
const PROCESS_METRICS_SEND_INTERVAL_MS: u64 = 500;
const FILE_SIZE_NOT_CHANGING_PERIOD_MS: u64 = 1000 * 60;
const LOG_FORWARD_ENDPOINT_DEV: &str = "https://sandbox.tracer.cloud/api/logs-forward/dev";
const LOG_FORWARD_ENDPOINT_PROD: &str = "https://sandbox.tracer.cloud/api/logs-forward/prod";
const SENTRY_DSN: &str = "https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248";
const GRAFANA_WORKSPACE_URL: &str = "https://tracerbio.grafana.net/goto/mYJ52c-HR?orgId=1";
const AWS_REGION: &str = "us-east-2";

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
    pub fn load_config() -> Result<Config> {
        Self::load_default_config()
    }

    /// Creates and returns a default configuration without looking for config files
    pub fn load_default_config() -> Result<Config> {
        let mut builder = RConfig::builder()
            .set_default("api_key", DEFAULT_API_KEY)?
            .set_default("process_polling_interval_ms", PROCESS_POLLING_INTERVAL_MS)?
            .set_default("batch_submission_interval_ms", BATCH_SUBMISSION_INTERVAL_MS)?
            .set_default(
                "process_metrics_send_interval_ms",
                PROCESS_METRICS_SEND_INTERVAL_MS,
            )?
            .set_default(
                "file_size_not_changing_period_ms",
                FILE_SIZE_NOT_CHANGING_PERIOD_MS,
            )?
            .set_default("new_run_pause_ms", NEW_RUN_PAUSE_MS)?
            .set_default("targets", targets_list::TARGETS.to_vec())?
            .set_default("aws_init_type", AwsConfig::Env)?
            .set_default("aws_region", AwsRegion::from(AWS_REGION))?
            .set_default("database_name", "tracer_db")?
            .set_default("grafana_workspace_url", GRAFANA_WORKSPACE_URL)?
            .set_default("server", "127.0.0.1:3000")?
            .set_default("sentry_dsn", SENTRY_DSN)?
            .set_default("log_forward_endpoint_dev", LOG_FORWARD_ENDPOINT_DEV)?
            .set_default("log_forward_endpoint_prod", LOG_FORWARD_ENDPOINT_PROD)?;

        // Build the config
        let config = builder.build()?;

        // Convert to our Config type
        let mut config: Config = config.try_deserialize()?;

        // Set config_sources to indicate this is a default config
        config.config_sources = vec!["default".to_string()];

        Ok(config)
    }

    pub fn setup_aliases() -> Result<()> {
        let config = ConfigLoader::load_config(None)?;
        rewrite_interceptor_bashrc_file(
            env::current_exe()?,
            config
                .targets
                .iter()
                .filter(|target| {
                    matches!(
                        &target.match_type,
                        TargetMatch::ShortLivedProcessExecutable(_)
                    )
                })
                .collect(),
        )?;

        modify_bashrc_file(".bashrc")?;

        println!("Command interceptors setup successfully.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile;

    #[test]
    fn test_load_config() {
        let config = ConfigLoader::load_config(None).unwrap();
        assert_eq!(config.api_key, DEFAULT_API_KEY);
    }
}
