use crate::server::DaemonServer;
use anyhow::{Context, Result};
use std::net::SocketAddr;
use tracer_client::config_manager::Config;
use tracer_client::exporters::db::AuroraClient;
use tracer_client::exporters::log_forward::LogForward;
use tracer_client::exporters::log_writer::LogWriterEnum;
use tracer_client::params::TracerCliInitArgs;
use tracer_client::TracerClient;
use tracing::info;

#[tokio::main]
pub async fn run(
    workflow_directory_path: String,
    cli_config_args: TracerCliInitArgs,
    config: Config,
) -> Result<()> {
    // create the conn pool to aurora
    let db_client = if config.log_forward_endpoint_dev.is_none() {
        LogWriterEnum::Aurora(AuroraClient::try_new(&config, None).await?)
    } else {
        println!("cli_config_args: {:?}", &cli_config_args);
        // if we pass --is-dev=false, we use the prod endpoint
        // if we pass --is-dev=true or don't pass the value, we use the dev endpoint
        let forward_endpoint =
            if cli_config_args.is_dev.is_some() && cli_config_args.is_dev.unwrap().eq(&false) {
                println!(
                    "Using prod endpoint: {}",
                    &config.log_forward_endpoint_prod.as_ref().unwrap()
                );
                &config.log_forward_endpoint_prod.as_ref().unwrap()
            } else {
                println!(
                    "Using dev endpoint: {}",
                    &config.log_forward_endpoint_dev.as_ref().unwrap()
                );
                &config.log_forward_endpoint_dev.as_ref().unwrap()
            };

        LogWriterEnum::Forward(LogForward::try_new(forward_endpoint).await?)
    };

    info!("Using {}", db_client.variant_name());

    let addr: SocketAddr = config.server.parse()?;

    let client = TracerClient::new(config, workflow_directory_path, db_client, cli_config_args)
        .await
        .context("Failed to create TracerClient")?;

    info!("Pipeline Name: {:?}", client.get_pipeline_name());
    DaemonServer::bind(client, addr).await?.run().await
}

pub async fn monitor_processes_with_tracer_client(tracer_client: &mut TracerClient) -> Result<()> {
    tracer_client.poll_process_metrics().await?;
    tracer_client.poll_syslog().await?;
    tracer_client.poll_stdout_stderr().await?;
    tracer_client.refresh_sysinfo().await?;
    // tracer_client.reset_just_started_process_flag().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::daemon::monitor_processes_with_tracer_client;
    use dotenv::dotenv;
    use std::path::Path;
    use tracer_client::config_manager::{Config, ConfigLoader};
    use tracer_client::exporters::db::AuroraClient;
    use tracer_client::exporters::log_forward::LogForward;
    use tracer_client::exporters::log_writer::{LogWriter, LogWriterEnum};
    use tracer_client::params::TracerCliInitArgs;
    use tracer_client::TracerClient;

    fn load_test_config() -> Config {
        let path = Path::new("../../");
        ConfigLoader::load_config_at(path, None).unwrap()
    }

    pub fn setup_env_vars(region: &str) {
        dotenv().ok(); // Load from .env file in development
        std::env::set_var("AWS_REGION", region);
    }

    #[tokio::test]
    async fn test_monitor_processes_with_tracer_client() -> Result<(), anyhow::Error> {
        let config = load_test_config();
        let pwd = std::env::current_dir()?;
        let region = "us-east-2";

        setup_env_vars(region);

        // let aurora_client: dyn LogWriter = AuroraClient::try_new(&config, None).await.unwrap();

        let log_forward_client = LogWriterEnum::Forward(
            LogForward::try_new(&config.log_forward_endpoint_dev.clone().unwrap())
                .await
                .expect("Failed to create LogForward"),
        );

        let mut tracer_client = TracerClient::new(
            config,
            pwd.to_str().unwrap().to_string(),
            log_forward_client,
            TracerCliInitArgs::default(),
        )
        .await
        .unwrap();
        let result = monitor_processes_with_tracer_client(&mut tracer_client).await;
        if result.is_ok() {
            Ok(result?)
        } else {
            Err(result.unwrap_err())
        }
    }
}
