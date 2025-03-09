/// lib.rs
//
pub mod cli;
pub mod cloud_providers;
pub mod config_manager;
pub mod daemon_communication;
pub mod events;
pub mod exporters;
pub mod extracts;

pub mod tracer_client;
pub mod types;
pub mod utils;
use anyhow::{Context, Ok, Result};
use daemonize::Daemonize;
use exporters::db::AuroraClient;
use types::cli::TracerCliInitArgs;

use std::fs::File;
use std::sync::Arc;

use crate::config_manager::ConfigManager;
use crate::tracer_client::TracerClient;

const PID_FILE: &str = "/tmp/tracerd.pid";
const WORKING_DIR: &str = "/tmp";
const STDOUT_FILE: &str = "/tmp/tracerd.out";
const STDERR_FILE: &str = "/tmp/tracerd.err";
const SOCKET_PATH: &str = "/tmp/tracerd.sock";
const FILE_CACHE_DIR: &str = "/tmp/tracerd_cache";

const SYSLOG_FILE: &str = "/var/log/syslog";

const REPO_OWNER: &str = "davincios";
const REPO_NAME: &str = "tracer-daemon";

// TODO: remove dependency from Service url completely
pub const DEFAULT_SERVICE_URL: &str = "https://app.tracer.bio/api";

pub fn start_daemon() -> Result<()> {
    //ConfigManager::test_service_config_sync()?;

    let daemon = Daemonize::new();
    daemon
        .pid_file(PID_FILE)
        .working_directory(WORKING_DIR)
        .stdout(
            File::create(STDOUT_FILE)
                .context("Failed to create stdout file")
                .unwrap(),
        )
        .stderr(
            File::create(STDERR_FILE)
                .context("Failed to create stderr file")
                .unwrap(),
        )
        .start()
        .context("Failed to start daemon.")
}

#[tokio::main]
pub async fn run(
    workflow_directory_path: String,
    cli_config_args: TracerCliInitArgs,
) -> Result<()> {
    let raw_config = ConfigManager::load_config();

    // create the conn pool to aurora
    let db_client = Arc::new(AuroraClient::new(&raw_config, None).await);

    let client = TracerClient::new(
        raw_config.clone(),
        workflow_directory_path,
        db_client,
        cli_config_args,
    )
    .await
    .context("Failed to create TracerClient")?;

    client.run().await
}

pub async fn monitor_processes_with_tracer_client(tracer_client: &mut TracerClient) -> Result<()> {
    tracer_client.remove_completed_processes().await?;
    tracer_client.poll_processes()?;
    // tracer_client.run_cleanup().await?;
    tracer_client.poll_process_metrics().await?;
    tracer_client.poll_syslog().await?;
    tracer_client.poll_stdout_stderr().await?;
    tracer_client.refresh_sysinfo();
    tracer_client.reset_just_started_process_flag();
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        config_manager::{Config, ConfigManager},
        exporters::db::AuroraClient,
        types::cli::TracerCliInitArgs,
    };

    use std::sync::Arc;

    use crate::{monitor_processes_with_tracer_client, TracerClient};
    use dotenv::dotenv;

    fn load_test_config() -> Config {
        ConfigManager::load_default_config()
    }

    pub fn setup_env_vars(region: &str) {
        dotenv().ok(); // Load from .env file in development
        std::env::set_var("AWS_REGION", region);
    }

    #[tokio::test]
    async fn test_monitor_processes_with_tracer_client() {
        let config = load_test_config();
        let pwd = std::env::current_dir().unwrap();
        let region = "us-east-2";

        setup_env_vars(region);

        let aurora_client = Arc::new(AuroraClient::new(&config, None).await);

        let mut tracer_client = TracerClient::new(
            config,
            pwd.to_str().unwrap().to_string(),
            aurora_client,
            TracerCliInitArgs::default(),
        )
        .await
        .unwrap();
        let result = monitor_processes_with_tracer_client(&mut tracer_client).await;
        assert!(result.is_ok());
    }
}
