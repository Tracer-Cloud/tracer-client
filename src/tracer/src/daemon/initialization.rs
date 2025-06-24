use crate::client::exporters::db::AuroraClient;
use crate::client::exporters::log_forward::LogForward;
use crate::client::exporters::log_writer::LogWriterEnum;
use crate::client::TracerClient;
use crate::config::Config;
use crate::daemon::server::DaemonServer;
use crate::process_identification::types::cli::params::FinalizedInitArgs;
use crate::utils::analytics::emit_analytic_event;
use anyhow::Context;
use tracing::info;

async fn get_db_client(init_args: &FinalizedInitArgs, config: &Config) -> LogWriterEnum {
    if config.log_forward_endpoint_dev.is_none() {
        LogWriterEnum::Aurora(AuroraClient::try_new(config, None).await.unwrap())
    } else {
        println!("cli_config_args: {:?}", init_args);
        // if we pass --is-dev=false, we use the prod endpoint
        // if we pass --is-dev=true or don't pass the value, we use the dev endpoint
        let forward_endpoint = &config.log_forward_endpoint_prod.as_ref().unwrap(); //TODO remove
                                                                                    // if cli_config_args.is_dev.is_some() && cli_config_args.is_dev.unwrap().eq(&false) {
                                                                                    //     println!(
                                                                                    //         "Using prod endpoint: {}",
                                                                                    //         &config.log_forward_endpoint_prod.as_ref().unwrap()
                                                                                    //     );
                                                                                    //     &config.log_forward_endpoint_prod.as_ref().unwrap()
                                                                                    // } else {
                                                                                    //     println!(
                                                                                    //         "Using dev endpoint: {}",
                                                                                    //         &config.log_forward_endpoint_dev.as_ref().unwrap()
                                                                                    //     );
                                                                                    //     &config.log_forward_endpoint_dev.as_ref().unwrap()
                                                                                    // };

        LogWriterEnum::Forward(LogForward::try_new(forward_endpoint).await.unwrap())
    }
}

async fn create_server(cli_config_args: FinalizedInitArgs, config: Config) -> DaemonServer {
    // create the conn pool to aurora
    let db_client = get_db_client(&cli_config_args, &config).await;

    info!("Using {}", db_client.variant_name());

    let client = TracerClient::new(config, db_client, cli_config_args)
        .await
        .context("Failed to create TracerClient")
        .unwrap();

    info!("Pipeline Name: {:?}", client.get_pipeline_name());
    // Push analytics event
    tokio::spawn(emit_analytic_event(
        client.user_id.clone(),
        crate::process_identification::types::analytics::AnalyticsEventType::DaemonStartedSuccessfully,
        None,
    ));

    let server = DaemonServer::new(client).await;
    info!("Daemon server created!");
    server
}

#[tokio::main]
pub async fn create_and_run_server(cli_config_args: FinalizedInitArgs, config: Config) {
    let server = create_server(cli_config_args, config).await;
    server.run().await.unwrap();
}