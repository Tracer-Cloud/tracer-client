use crate::cli::handlers::arguments::FinalizedInitArgs;
use crate::client::exporters::log_forward::LogForward;
use crate::client::exporters::log_writer::LogWriterEnum;
use crate::client::TracerClient;
use crate::config::Config;
use crate::daemon::server::DaemonServer;
use crate::utils::analytics;
use crate::utils::analytics::types::AnalyticsEventType;
use anyhow::Context;
use tracing::info;

async fn get_db_client(init_args: &FinalizedInitArgs, config: &Config) -> LogWriterEnum {
    // if we pass --is-dev=false, we use the prod endpoint
    // if we don't pass any value, we use the prod endpoint
    // if we pass --is-dev=true, we use the dev endpoint
    // dev endpoint points to clickhouse, prod endpoint points to postgres
    let log_forward_endpoint = if init_args.is_dev.unwrap_or(false) {
        println!(
            "Using dev endpoint: {}",
            &config.log_forward_endpoint_dev.as_ref().unwrap()
        );
        &config.log_forward_endpoint_dev.as_ref().unwrap()
    } else {
        println!(
            "Using prod endpoint: {}",
            &config.log_forward_endpoint_prod.as_ref().unwrap()
        );
        &config.log_forward_endpoint_prod.as_ref().unwrap()
    };

    LogWriterEnum::Forward(LogForward::try_new(log_forward_endpoint).await.unwrap())
}

async fn create_server(cli_config_args: FinalizedInitArgs, config: Config) -> DaemonServer {
    // create the connection pool to aurora
    let db_client = get_db_client(&cli_config_args, &config).await;

    info!("Using {}", db_client.variant_name());

    let client = TracerClient::new(config, db_client, cli_config_args)
        .await
        .context("Failed to create TracerClient")
        .unwrap();

    info!("Pipeline Name: {:?}", client.get_pipeline_name());
    // Push analytics event
    analytics::spawn_event(
        client.user_id.clone(),
        AnalyticsEventType::DaemonStartedSuccessfully,
        None,
    );

    let server = DaemonServer::new(client).await;
    info!("Daemon server created!");
    server
}

pub async fn create_and_run_server(
    cli_config_args: FinalizedInitArgs,
    config: Config,
) -> anyhow::Result<()> {
    let server = create_server(cli_config_args, config).await;
    server.run().await
}
