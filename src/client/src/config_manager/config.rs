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

    pub config_sources: Vec<String>,
    pub sentry_dsn: Option<String>,
}

pub struct ConfigLoader;

impl ConfigLoader {
    /// Search for config in TRACER_CONFIG_DIR, ~/.config/tracer, and/or working directory
    fn get_config_path(config_name: Option<&str>) -> Result<PathBuf> {
        // Get list of dirs to search
        let mut dirs_to_search = Vec::new();
        if let Ok(env_dir) = env::var("TRACER_CONFIG_DIR") {
            dirs_to_search.push(PathBuf::from(env_dir));
        } else {
            // If no directory explicitly specified
            if let Ok(Some(home)) = homedir::get_my_home() {
                // TODO: skip HOME when unit testing?
                dirs_to_search.push(
                    home.join(DEFAULT_CONFIG_FILE_LOCATION_FROM_HOME)
                        .to_path_buf(),
                );
            }
            dirs_to_search.push(env::current_dir()?.canonicalize()?);
        }

        // Try to find config in each dir
        for dir in &dirs_to_search {
            if let Ok(config_name) = Self::find_config_at(dir, config_name) {
                return Ok(dir.join(config_name));
            }
        }

        // If none found, default to first entry in dirs_to_search
        if let Some(first_dir) = dirs_to_search.first() {
            return Ok(first_dir.join("tracer.toml"));
        }

        anyhow::bail!("No valid configuration path found")
    }

    /// Search for config file in `dir` matching `config_name` (or defaults).
    fn find_config_at(dir: &Path, config_name: Option<&str>) -> Result<String> {
        if let Some(name) = config_name {
            // find all .toml files containing the substring
            let mut candidates = Vec::new();
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let fname = entry.file_name().to_string_lossy().into_owned();
                if fname.ends_with(".toml") && fname.contains(name) {
                    candidates.push(fname);
                }
            }
            match candidates.len() {
                1 => Ok(candidates.remove(0)),
                0 => anyhow::bail!("No config matching '{}' in {:?}", name, dir),
                _ => anyhow::bail!(
                    "Multiple configs matching '{}' in {:?}: {:?}",
                    name,
                    dir,
                    candidates
                ),
            }
        } else if dir.to_str().is_some_and(|s| s.ends_with(".toml")) {
            return Ok(dir.file_name().unwrap().to_string_lossy().into_owned());
        } else {
            // default order
            let defaults = [
                "tracer.production.toml",
                "tracer.development.toml",
                "tracer.toml",
            ];
            for &fname in &defaults {
                let candidate = dir.join(fname);
                if candidate.is_file() {
                    return Ok(fname.to_string());
                }
            }
            anyhow::bail!("No default config file found in {:?}", dir);
        }
    }

    pub fn load_config(config_name: Option<&str>) -> Result<Config> {
        let path = Self::get_config_path(config_name)?;
        let dir = path.parent().context("Failed to get parent directory")?;
        Self::load_config_at(dir, config_name)
    }

    pub fn load_config_at(path: &Path, config_name: Option<&str>) -> Result<Config> {
        let chosen = Self::find_config_at(path, config_name)?;
        let chosen_path = path.join(&chosen);
        let chosen_overrides_path = path.join(chosen.replace(".toml", ".local.toml"));
        info!("Using config file: {:?}", chosen_path);

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

        // load toml & envar config
        let mut builder = RConfig::builder()
            .add_source(
                File::with_name(chosen_path.to_str().context("invalid path")?).required(false),
            )
            .add_source(
                File::with_name(chosen_overrides_path.to_str().context("invalid path")?)
                    .required(false),
            )
            .add_source(
                Environment::with_prefix("TRACER")
                    .convert_case(Case::Snake)
                    .separator("__")
                    .prefix_separator("_"),
            );

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
            .set_default("aws_region", "us-east-2")?
            .set_default("database_name", "tracer_db")?
            .set_default("server", "127.0.0.1:8722")?
            .set_default::<&str, Vec<&str>>("targets", vec![])?;

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

        if chosen_path.is_file() {
            config
                .config_sources
                .push(chosen_path.to_string_lossy().into_owned());
        }
        if chosen_overrides_path.is_file() {
            config
                .config_sources
                .push(chosen_overrides_path.to_string_lossy().into_owned());
        }

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

    pub fn save_config(config: &Config) -> Result<()> {
        // todo: should this be async? should others be async?
        let config_file_location = config.config_sources.first().cloned().or_else(|| {
            ConfigLoader::get_config_path(None)
                .ok()
                .map(|path| path.to_string_lossy().into_owned())
        });

        if let Some(location) = config_file_location {
            let config = toml::to_string(config)?;
            std::fs::write(location, config)?;
        } else {
            anyhow::bail!("Failed to determine config file location");
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
        let config = ConfigLoader::load_config_at(path, None).unwrap();
        assert!(!config.targets.is_empty());
    }

    // Test: exactly one matching file → should load successfully
    #[test]
    fn test_search_exact_one_match() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let dir_path = temp_dir.path();

        // Create a single "*.toml" containing "unique" in its name
        let file_name = "unique_config.toml";
        let file_path = dir_path.join(file_name);
        // Give it a minimal valid setting to override the default
        std::fs::write(
            &file_path,
            r#"
            api_key = "123"
            process_polling_interval_ms = 123
            batch_submission_interval_ms = 123
            database_secrets_arn = "123"
            database_host = "123"
            database_name = "123"
            grafana_workspace_url = "123"
            server = "123"
        "#,
        )
        .expect("failed to write toml file");

        // Should find exactly that one file and load it
        let cfg = ConfigLoader::load_config_at(dir_path, Some("unique_config"))
            .expect("should load config with one match");
        assert_eq!(cfg.api_key, "123");
    }

    // Test: no matching files → should error out
    #[test]
    fn test_search_no_match() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let dir_path = temp_dir.path();

        // Create a tangential file that does *not* contain "missing"
        std::fs::write(dir_path.join("other.toml"), "").expect("write dummy file");

        // Asking for "missing" should produce a "No config matching" error
        let err = ConfigLoader::load_config_at(dir_path, Some("missing")).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("No config matching 'missing'"),
            "unexpected error: {}",
            msg
        );
    }

    // Test: multiple matching files → should error out
    #[test]
    fn test_search_multiple_matches() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let dir_path = temp_dir.path();

        // Create two files both containing the substring "dup"
        std::fs::write(dir_path.join("dup_a.toml"), "").unwrap();
        std::fs::write(dir_path.join("dup_b.toml"), "").unwrap();

        // Asking for "dup" should detect two candidates and bail
        let err = ConfigLoader::load_config_at(dir_path, Some("dup")).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Multiple configs matching 'dup'"),
            "unexpected error: {}",
            msg
        );
    }
}
