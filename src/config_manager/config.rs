// src/config_manager/mod.rs
use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    config_manager::{
        bashrc_intercept::{modify_bashrc_file, rewrite_interceptor_bashrc_file},
        target_process::target_matching::TargetMatch,
    },
    events::send_daemon_start_event,
    types::{aws::aws_region::AwsRegion, config::AwsConfig},
};

use crate::config_manager::target_process::Target;

use super::target_process::targets_list;

const DEFAULT_API_KEY: &str = "EAjg7eHtsGnP3fTURcPz1";
const DEFAULT_SERVICE_URL: &str = "https://app.tracer.bio/api";
const DEFAULT_CONFIG_FILE_LOCATION_FROM_HOME: &str = ".config/tracer/tracer.toml";
const PROCESS_POLLING_INTERVAL_MS: u64 = 5;
const BATCH_SUBMISSION_INTERVAL_MS: u64 = 10000;
const NEW_RUN_PAUSE_MS: u64 = 10 * 60 * 1000;
const PROCESS_METRICS_SEND_INTERVAL_MS: u64 = 10000;
const FILE_SIZE_NOT_CHANGING_PERIOD_MS: u64 = 1000 * 60;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConfigFile {
    pub api_key: String,
    pub service_url: Option<String>,
    pub process_polling_interval_ms: Option<u64>,
    pub batch_submission_interval_ms: Option<u64>,
    pub new_run_pause_ms: Option<u64>,
    pub file_size_not_changing_period_ms: Option<u64>,
    pub process_metrics_send_interval_ms: Option<u64>,
    pub targets: Option<Vec<Target>>,
    pub aws_region: Option<String>,
    pub aws_role_arn: Option<String>,
    pub aws_profile: Option<String>,
    pub database_secrets_arn: String,
    pub database_host: String,
    pub database_name: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub api_key: String,
    pub process_polling_interval_ms: u64,
    pub batch_submission_interval_ms: u64,
    pub process_metrics_send_interval_ms: u64,
    pub file_size_not_changing_period_ms: u64,
    pub service_url: String,
    pub new_run_pause_ms: u64,
    pub targets: Vec<Target>,
    pub aws_init_type: AwsConfig,
    pub aws_region: AwsRegion,

    pub database_secrets_arn: String,
    pub database_host: String,
    pub database_name: String,
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

    fn load_config_from_file(path: &PathBuf) -> Result<Config> {
        let config = std::fs::read_to_string(path)?;
        let config: ConfigFile = toml::from_str(&config)?;

        let aws_init_type = match (config.aws_role_arn, config.aws_profile) {
            (Some(role), None) => AwsConfig::RoleArn(role),
            (None, Some(profile)) => AwsConfig::Profile(profile),
            (Some(role), Some(_profie)) => AwsConfig::RoleArn(role),
            (None, None) => AwsConfig::Env,
        };

        Ok(Config {
            api_key: config.api_key,
            process_polling_interval_ms: config
                .process_polling_interval_ms
                .unwrap_or(PROCESS_POLLING_INTERVAL_MS),
            batch_submission_interval_ms: config
                .batch_submission_interval_ms
                .unwrap_or(BATCH_SUBMISSION_INTERVAL_MS),
            service_url: config
                .service_url
                .unwrap_or(DEFAULT_SERVICE_URL.to_string()),
            new_run_pause_ms: config.new_run_pause_ms.unwrap_or(NEW_RUN_PAUSE_MS),
            process_metrics_send_interval_ms: config
                .process_metrics_send_interval_ms
                .unwrap_or(PROCESS_METRICS_SEND_INTERVAL_MS),
            file_size_not_changing_period_ms: config
                .file_size_not_changing_period_ms
                .unwrap_or(FILE_SIZE_NOT_CHANGING_PERIOD_MS),
            targets: config
                .targets
                .unwrap_or_else(|| targets_list::TARGETS.to_vec()),
            aws_init_type,
            aws_region: AwsRegion::UsEast2,

            database_secrets_arn: config.database_secrets_arn,
            database_name: config.database_name,
            database_host: config.database_host,
        })
    }

    pub fn load_default_config() -> Config {
        Config {
            api_key: DEFAULT_API_KEY.to_string(),
            process_polling_interval_ms: PROCESS_POLLING_INTERVAL_MS,
            batch_submission_interval_ms: BATCH_SUBMISSION_INTERVAL_MS,
            new_run_pause_ms: NEW_RUN_PAUSE_MS,
            file_size_not_changing_period_ms: FILE_SIZE_NOT_CHANGING_PERIOD_MS,
            service_url: DEFAULT_SERVICE_URL.to_string(),
            targets: targets_list::TARGETS.to_vec(),
            process_metrics_send_interval_ms: PROCESS_METRICS_SEND_INTERVAL_MS,
            // aws_init_type: AwsConfig::Profile("me".to_string()),
            aws_init_type: AwsConfig::Profile(
                if std::fs::read_to_string(dirs::home_dir().unwrap().join(".aws/credentials"))
                    .unwrap_or_default()
                    .contains("[me]")
                {
                    "me"
                } else {
                    "default"
                }
                .to_string(),
            ),
            aws_region: "us-east-2".into(),

            database_secrets_arn: "invalid".into(),
            database_name: "postgres".into(),
            database_host: "tracer-database.cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432".into(),
        }
    }

    // TODO: add error message as to why it can't read config
    pub fn load_config() -> Config {
        let config_file_location = ConfigManager::get_config_path();

        let mut config = if let Some(path) = config_file_location {
            let loaded_config = ConfigManager::load_config_from_file(&path);
            if loaded_config.is_err() {
                println!(
                    "\nFailed to load config from {:?}, using default config.\n",
                    path
                )
            }
            loaded_config.unwrap_or_else(|_| ConfigManager::load_default_config())
        } else {
            ConfigManager::load_default_config()
        };

        if let Ok(api_key) = std::env::var("TRACER_API_KEY") {
            config.api_key = api_key;
        }

        if let Ok(service_url) = std::env::var("TRACER_SERVICE_URL") {
            config.service_url = service_url;
        }

        config.service_url = config.service_url.replace("data-collector-api", ""); // To support legacy (pre-2024/08/23) configs

        config
    }

    pub fn setup_aliases() -> Result<()> {
        let config = ConfigManager::load_config();
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
        let config_file_location = ConfigManager::get_config_path().unwrap();
        let aws_profile = if let AwsConfig::Profile(profile) = &config.aws_init_type {
            Some(profile.clone())
        } else {
            None
        };
        let aws_role_arn = if let AwsConfig::RoleArn(role) = &config.aws_init_type {
            Some(role.clone())
        } else {
            None
        };
        let config_out = ConfigFile {
            api_key: config.api_key.clone(),
            service_url: Some(config.service_url.clone()),
            new_run_pause_ms: Some(config.new_run_pause_ms),
            file_size_not_changing_period_ms: Some(config.file_size_not_changing_period_ms),
            process_polling_interval_ms: Some(config.process_polling_interval_ms),
            batch_submission_interval_ms: Some(config.batch_submission_interval_ms),
            targets: Some(config.targets.clone()),
            process_metrics_send_interval_ms: Some(config.process_metrics_send_interval_ms),
            aws_role_arn,
            aws_profile,
            aws_region: Some(config.aws_region.as_str().to_string()),

            database_secrets_arn: config.database_secrets_arn.clone(),
            database_name: config.database_name.clone(),
            database_host: config.database_host.clone(),
        };
        let config = toml::to_string(&config_out)?;
        std::fs::write(config_file_location, config)?;
        Ok(())
    }

    pub fn modify_config(
        api_key: &Option<String>,
        service_url: &Option<String>,
        process_polling_interval_ms: &Option<u64>,
        batch_submission_interval_ms: &Option<u64>,
    ) -> Result<()> {
        let mut current_config = ConfigManager::load_config();
        if let Some(api_key) = api_key {
            current_config.api_key.clone_from(api_key);
        }
        if let Some(service_url) = service_url {
            current_config.service_url.clone_from(service_url);
        }
        if let Some(process_polling_interval_ms) = process_polling_interval_ms {
            current_config.process_polling_interval_ms = *process_polling_interval_ms;
        }
        if let Some(batch_submission_interval_ms) = batch_submission_interval_ms {
            current_config.batch_submission_interval_ms = *batch_submission_interval_ms;
        }
        ConfigManager::save_config(&current_config)?;
        Ok(())
    }

    // TODO: remove dependencies away from frontend api
    pub async fn test_service_config() -> Result<()> {
        let config = ConfigManager::load_config();

        let result = send_daemon_start_event(&config.service_url, &config.api_key).await;

        if let Err(error) = result {
            println!("Failed to test the service configuration! Please check the configuration and try again.");
            println!("{}", &error);
            return Err(error);
        }

        Ok(())
    }

    pub fn _test_service_config_sync() -> Result<()> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(ConfigManager::test_service_config())
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
    use std::env;
    use tempfile;

    #[test]
    fn test_default_config() {
        env::remove_var("TRACER_API_KEY");
        env::remove_var("TRACER_SERVICE_URL");
        let config = ConfigManager::load_default_config();
        assert_eq!(config.api_key, DEFAULT_API_KEY);
        assert_eq!(config.service_url, DEFAULT_SERVICE_URL);
        assert_eq!(
            config.process_polling_interval_ms,
            PROCESS_POLLING_INTERVAL_MS
        );
        assert_eq!(
            config.batch_submission_interval_ms,
            BATCH_SUBMISSION_INTERVAL_MS
        );
        assert_eq!(
            config.process_metrics_send_interval_ms,
            PROCESS_METRICS_SEND_INTERVAL_MS
        );
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
