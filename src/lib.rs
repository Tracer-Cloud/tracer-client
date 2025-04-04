/// lib.rs
//
pub mod cli;
pub mod cloud_providers;
pub mod config_manager;
pub mod daemon_communication;
pub mod events;
pub mod exporters;
pub mod extracts;

mod nextflow_log_watcher;
pub mod server;
pub mod tracer_client;
pub mod types;
pub mod utils;

use anyhow::{Context, Result};
use daemonize::Daemonize;
use exporters::db::AuroraClient;
use std::fs::File;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, time::SystemTime},
    prelude::*,
    EnvFilter,
};
use types::cli::TracerCliInitArgs;

use crate::config_manager::Config;
use crate::server::TracerServer;
use crate::tracer_client::TracerClient;

const WORKING_DIR: &str = "/tmp/tracer/";
const PID_FILE: &str = "/tmp/tracer/tracerd.pid";
const STDOUT_FILE: &str = "/tmp/tracer/tracerd.out";
const STDERR_FILE: &str = "/tmp/tracer/tracerd.err";
const LOG_FILE: &str = "/tmp/tracer/daemon.log";
const FILE_CACHE_DIR: &str = "/tmp/tracer/tracerd_cache";
const DEBUG_LOG: &str = "/tmp/tracer/debug.log";

const SYSLOG_FILE: &str = "/var/log/syslog";

const REPO_OWNER: &str = "davincios";
const REPO_NAME: &str = "tracer-daemon";

// TODO: remove dependency from Service url completely
pub const DEFAULT_SERVICE_URL: &str = "https://app.tracer.bio/api";

pub fn start_daemon() -> Result<()> {
    let _ = std::fs::create_dir_all(WORKING_DIR);

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
        .umask(0o002)
        .start()
        .context("Failed to start daemon.")
}

#[tokio::main]
pub async fn run(
    workflow_directory_path: String,
    cli_config_args: TracerCliInitArgs,
    config: Config,
) -> Result<()> {
    // Set up logging first
    setup_logging()?;

    // create the conn pool to aurora
    let db_client = Arc::new(AuroraClient::new(&config, None).await);

    let client = TracerClient::new(
        config.clone(),
        workflow_directory_path,
        db_client,
        cli_config_args,
    )
    .await
    .context("Failed to create TracerClient")?;

    println!("Pipeline Name: {:?}", client.get_pipeline_name());

    todo!();
    // let addr = SocketAddr::par;
    // TracerServer::bind(client, )
}

fn setup_logging() -> Result<()> {
    // Set up the filter
    let filter = EnvFilter::from("debug"); // Capture all levels from debug up

    // Create a file appender that writes to daemon.log
    let file_appender = RollingFileAppender::new(Rotation::NEVER, WORKING_DIR, "daemon.log");

    // Create a custom format for the logs
    let file_layer = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(true)
        .with_level(true)
        .with_timer(SystemTime)
        .with_writer(file_appender);

    // Set up the subscriber with our custom layer
    let subscriber = tracing_subscriber::registry().with(filter).with(file_layer);

    // Set the subscriber as the default
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    // Log initialization message
    tracing::info!("Logging system initialized. Writing to {}", LOG_FILE);

    Ok(())
}

pub async fn monitor_processes_with_tracer_client(tracer_client: &mut TracerClient) -> Result<()> {
    tracer_client.remove_completed_processes().await?;
    tracer_client.poll_processes()?;
    // tracer_client.run_cleanup().await?;
    tracer_client.poll_process_metrics().await?;
    tracer_client.poll_syslog().await?;
    tracer_client.poll_stdout_stderr().await?;
    tracer_client.poll_nextflow_log().await?;
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
