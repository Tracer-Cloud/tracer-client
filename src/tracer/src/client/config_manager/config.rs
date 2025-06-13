use std::env;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::client::config_manager::bashrc_intercept::{
    modify_bashrc_file, rewrite_interceptor_bashrc_file,
};

use crate::cloud_providers::aws::config::AwsConfig;
use crate::cloud_providers::aws::types::aws_region::AwsRegion;
use crate::common::target_process::target_matching::TargetMatch;
use crate::common::target_process::targets_list;
use crate::common::target_process::Target;
use config::Config as RConfig;

const DEFAULT_API_KEY: &str = "EAjg7eHtsGnP3fTURcPz1";
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

pub const TRACER_ANALYTICS_ENDPOINT: &str = "https://sandbox.tracer.cloud/api/analytics";

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
    pub fn load_default_config() -> Result<Config> {
        let aws_default_profile = match dirs::home_dir() {
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
        .to_string();

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
            .set_default("aws_init_type", AwsConfig::Profile(aws_default_profile))?
            .set_default("aws_region", AWS_REGION)?
            .set_default("database_name", "tracer_db")?
            .set_default("server", "127.0.0.1:8722")?
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

    pub fn setup_aliases() -> Result<()> {
        let config = ConfigLoader::load_default_config()?;
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

    #[test]
    fn test_default_config() {
        let config = ConfigLoader::load_default_config().unwrap();
        assert!(!config.targets.is_empty());
    }
}
