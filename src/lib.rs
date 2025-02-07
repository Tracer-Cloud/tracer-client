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
use config_manager::{INTERCEPTOR_STDERR_FILE, INTERCEPTOR_STDOUT_FILE};
use daemon_communication::server::run_server;
use daemonize::Daemonize;
use extracts::syslog::run_syslog_lines_read_thread;
use std::borrow::BorrowMut;

use crate::exporters::{FsExportHandler, S3ExportHandler};
use std::fs::File;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tokio_util::sync::CancellationToken;

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

pub fn start_daemon() -> Result<()> {
    ConfigManager::test_service_config_sync()?;

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
pub async fn run(workflow_directory_path: String, pipeline_name: String) -> Result<()> {
    let raw_config = ConfigManager::load_config();

    let export_dir = ConfigManager::get_tracer_parquet_export_dir()?;

    let fs_handler = FsExportHandler::new(export_dir, None);
    let s3_handler = S3ExportHandler::new(
        fs_handler,
        raw_config.aws_init_type.clone(),
        raw_config.aws_region.as_str(),
    )
    .await;

    let client = TracerClient::new(
        raw_config.clone(),
        workflow_directory_path,
        s3_handler,
        pipeline_name,
    )
    .await
    .context("Failed to create TracerClient")?;
    let tracer_client = Arc::new(Mutex::new(client));
    let config: Arc<RwLock<config_manager::Config>> = Arc::new(RwLock::new(raw_config));

    let cancellation_token = CancellationToken::new();

    tokio::spawn(run_server(
        tracer_client.clone(),
        SOCKET_PATH,
        cancellation_token.clone(),
        config.clone(),
    ));

    let syslog_lines_task = tokio::spawn(run_syslog_lines_read_thread(
        SYSLOG_FILE,
        tracer_client.lock().await.get_syslog_lines_buffer(),
    ));

    let stdout_lines_task = tokio::spawn(extracts::stdout::run_stdout_lines_read_thread(
        INTERCEPTOR_STDOUT_FILE,
        INTERCEPTOR_STDERR_FILE,
        tracer_client.lock().await.get_stdout_stderr_lines_buffer(),
    ));

    tracer_client
        .lock()
        .await
        .borrow_mut()
        .start_new_run(None)
        .await?;

    while !cancellation_token.is_cancelled() {
        let start_time = Instant::now();
        while start_time.elapsed()
            < Duration::from_millis(config.read().await.batch_submission_interval_ms)
        {
            monitor_processes_with_tracer_client(tracer_client.lock().await.borrow_mut()).await?;
            sleep(Duration::from_millis(
                config.read().await.process_polling_interval_ms,
            ))
            .await;
            if cancellation_token.is_cancelled() {
                break;
            }
        }

        tracer_client
            .lock()
            .await
            .borrow_mut()
            .submit_batched_data()
            .await?;

        tracer_client.lock().await.borrow_mut().poll_files().await?;
    }

    syslog_lines_task.abort();
    stdout_lines_task.abort();

    Ok(())
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
    use crate::config_manager::{Config, ConfigManager};
    use crate::{
        monitor_processes_with_tracer_client, FsExportHandler, S3ExportHandler, TracerClient,
    };
    use aws_config::BehaviorVersion;
    use dotenv::dotenv;
    use tempdir::TempDir;

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

        let temp_dir = TempDir::new("export").expect("failed to create tempdir");
        let base_dir = temp_dir.path().join("./exports");
        let fs_handler = FsExportHandler::new(base_dir, None);

        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .load()
            .await;

        let s3_handler = S3ExportHandler::new_with_config(fs_handler, aws_config).await;

        let mut tracer_client = TracerClient::new(
            config,
            pwd.to_str().unwrap().to_string(),
            s3_handler,
            "testing".to_string(),
        )
        .await
        .unwrap();
        let result = monitor_processes_with_tracer_client(&mut tracer_client).await;
        assert!(result.is_ok());
    }
}
