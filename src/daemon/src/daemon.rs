use crate::server::DaemonServer;
use anyhow::{Context, Result};
use std::net::SocketAddr;
use tracer_client::config_manager::Config;
use tracer_client::exporters::db::AuroraClient;
use tracer_client::params::TracerCliInitArgs;
use tracer_client::TracerClient;
use tracer_common::constants::{LOG_FILE, WORKING_DIR};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, time::SystemTime},
    prelude::*,
    EnvFilter,
};

#[tokio::main]
pub async fn run(
    workflow_directory_path: String,
    cli_config_args: TracerCliInitArgs,
    config: Config,
) -> Result<()> {
    // Set up logging first
    setup_logging()?;

    // create the conn pool to aurora
    let db_client = AuroraClient::try_new(&config, None).await?;

    let addr: SocketAddr = config.server.parse()?;

    let client = TracerClient::new(config, workflow_directory_path, db_client, cli_config_args)
        .await
        .context("Failed to create TracerClient")?;

    println!("Pipeline Name: {:?}", client.get_pipeline_name());

    DaemonServer::bind(client, addr).await?.run().await
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
    tracer_client.poll_processes().await?;
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
    use crate::daemon::monitor_processes_with_tracer_client;
    use dotenv::dotenv;
    use std::path::Path;
    use tracer_client::config_manager::{Config, ConfigManager};
    use tracer_client::exporters::db::AuroraClient;
    use tracer_client::params::TracerCliInitArgs;
    use tracer_client::TracerClient;

    fn load_test_config() -> Config {
        let path = Path::new("../../");
        ConfigManager::load_config_at(path).unwrap()
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

        let aurora_client = AuroraClient::try_new(&config, None).await.unwrap();

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
