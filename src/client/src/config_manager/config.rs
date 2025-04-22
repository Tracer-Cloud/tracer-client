use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
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
const DEFAULT_CONFIG_FILE_LOCATION_FROM_HOME: &str = ".config/tracer/tracer.toml";
const PROCESS_POLLING_INTERVAL_MS: u64 = 5;
const NEXTFLOW_LOG_FILE_POLLING_INTERVAL_MS: u64 = 2000;
const BATCH_SUBMISSION_INTERVAL_MS: u64 = 10000;
const NEW_RUN_PAUSE_MS: u64 = 10 * 60 * 1000;
const PROCESS_METRICS_SEND_INTERVAL_MS: u64 = 10000;
const FILE_SIZE_NOT_CHANGING_PERIOD_MS: u64 = 1000 * 60;

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

    pub database_secrets_arn: String,
    pub database_host: String,
    pub database_name: String,

    pub grafana_workspace_url: String,
    pub server: String,
}

pub struct ConfigManager;

impl ConfigManager {
    fn get_config_path() -> Option<PathBuf> {
        let path = homedir::get_my_home();

        match path {
            Ok(Some(path)) => {
                let path = path.join(DEFAULT_CONFIG_FILE_LOCATION_FROM_HOME);
                Some(path)
            }
            _ => None,
        }
    }

    pub fn get_nextflow_log_polling_interval_ms() -> u64 {
        NEXTFLOW_LOG_FILE_POLLING_INTERVAL_MS
    }

    // TODO: add error message as to why it can't read config

    pub fn load_config() -> Result<Config> {
        if let Ok(path) = std::env::var("TRACER_CONFIG_DIR") {
            let path = Path::new(&path);
            ConfigManager::load_config_at(path)
        } else {
            let path = Path::new(".");
            ConfigManager::load_config_at(path)
        }
    }

    pub fn load_config_at(path: &Path) -> Result<Config> {
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

        let mut cb = RConfig::builder()
            .add_source(
                File::with_name(path.join("tracer.toml").to_str().context("Join path")?)
                    .required(false),
            )
            .add_source(
                File::with_name(path.join("tracer.dev.toml").to_str().context("Join path")?)
                    .required(false),
            )
            .add_source(
                Environment::with_prefix("TRACER")
                    .convert_case(Case::Snake)
                    .separator("__")
                    .prefix_separator("_"),
            )
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
            .set_default("aws_region", "us-east-2")?
            .set_default("database_name", "tracer_db")?
            .set_default("server", "127.0.0.1:8722")?
            .set_default::<&str, Vec<&str>>("targets", vec![])?;

        if let Some(path) = ConfigManager::get_config_path() {
            if let Some(path) = path.to_str() {
                cb = cb.add_source(File::with_name(path).required(false))
            }
        }

        let mut config: Config = cb
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
        let config = ConfigManager::load_config()?;
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
        // bashrc_intercept(".bashrc")?;
        modify_bashrc_file(".bashrc")?;

        println!("Command interceptors setup successfully.");
        Ok(())
    }

    pub fn save_config(config: &Config) -> Result<()> {
        // todo: should this be async? should others be async?
        let Some(config_file_location) = ConfigManager::get_config_path() else {
            anyhow::bail!("Failed to get config file location");
        };

        let config = toml::to_string(config)?;
        std::fs::write(config_file_location, config)?;
        Ok(())
    }

    pub fn modify_config(
        api_key: &Option<String>,
        process_polling_interval_ms: &Option<u64>,
        batch_submission_interval_ms: &Option<u64>,
    ) -> Result<()> {
        let mut current_config = ConfigManager::load_config()?;
        if let Some(api_key) = api_key {
            current_config.api_key.clone_from(api_key);
        }
        if let Some(process_polling_interval_ms) = process_polling_interval_ms {
            current_config.process_polling_interval_ms = *process_polling_interval_ms;
        }
        if let Some(batch_submission_interval_ms) = batch_submission_interval_ms {
            current_config.batch_submission_interval_ms = *batch_submission_interval_ms;
        }
        ConfigManager::save_config(&current_config)
    }

    pub fn get_tracer_parquet_export_dir() -> Result<PathBuf> {
        let mut export_dir = homedir::get_my_home()?.expect("Failed to get home dir");
        export_dir.push("exports");
        // Create export dir if not exists
        let _ = std::fs::create_dir_all(&export_dir);
        Self::validate_path(&export_dir)?;
        Ok(export_dir)
    }

    /// Validates a directory of file path. It checks if it exists or has write permissions
    pub fn validate_path<P: AsRef<Path>>(dir: P) -> Result<()> {
        let path = dir.as_ref();

        if !path.exists() {
            anyhow::bail!(format!("{path:?} is not a valid path"))
        }

        if path
            .metadata()
            .map_err(|err| {
                anyhow::anyhow!(
                    "Failed to get metadata for path {:?}. Error: {}",
                    path,
                    err.to_string()
                )
            })?
            .permissions()
            .readonly()
        {
            anyhow::bail!("Only Readonly permissions granted for path: {path:?}")
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile;

    #[test]
    fn test_default_config() {
        let path = Path::new("../../");
        let config = ConfigManager::load_config_at(path).unwrap();
        assert!(!config.targets.is_empty());
    }

    #[test]
    fn test_path_validation_for_dir_succeeds() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let dir_path = temp_dir.path();

        assert!(ConfigManager::validate_path(dir_path).is_ok());
    }

    #[test]
    fn test_path_validation_for_file_succeeds() {
        // Create a temporary directory
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test_file.txt");

        std::fs::File::create(&file_path).expect("failed to create file");

        assert!(ConfigManager::validate_path(file_path).is_ok());
    }

    #[test]
    fn test_path_validation_invalid_file() {
        let invalid_path = "non_existent_file.txt";
        assert!(ConfigManager::validate_path(invalid_path).is_err());
    }

    #[test]
    fn test_read_only_permissions() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("readonly_file.txt");
        std::fs::File::create(&file_path).expect("Failed to create temp file");

        // Set the file to readonly
        let mut permissions = std::fs::metadata(&file_path)
            .expect("Failed to get metadata")
            .permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&file_path, permissions)
            .expect("Failed to set readonly permissions");

        assert!(ConfigManager::validate_path(&file_path).is_err());
    }
}
