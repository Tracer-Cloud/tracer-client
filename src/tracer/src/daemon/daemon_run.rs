use crate::client::exporters::db::AuroraClient;
use crate::client::exporters::log_forward::LogForward;
use crate::client::exporters::log_writer::LogWriterEnum;
use crate::client::TracerClient;
use crate::common::types::cli::params::FinalizedInitArgs;
use crate::config::Config;
use crate::daemon::server::DaemonServer;
use crate::utils::analytics::emit_analytic_event;
use anyhow::{Context, Result};
use tracing::info;

#[tokio::main]
pub async fn run(cli_config_args: FinalizedInitArgs, config: Config) -> Result<()> {
    // create the conn pool to aurora
    let db_client = if config.log_forward_endpoint_dev.is_none() {
        LogWriterEnum::Aurora(AuroraClient::try_new(&config, None).await?)
    } else {
        println!("cli_config_args: {:?}", &cli_config_args);
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

        LogWriterEnum::Forward(LogForward::try_new(forward_endpoint).await?)
    };

    info!("Using {}", db_client.variant_name());
    
    let client = TracerClient::new(config, db_client, cli_config_args)
        .await
        .context("Failed to create TracerClient")?;

    info!("Pipeline Name: {:?}", client.get_pipeline_name());
    tokio::spawn(emit_analytic_event(
        client.user_id.clone(),
        crate::common::types::analytics::AnalyticsEventType::DaemonStartedSuccessfully,
        None,
    ));
    DaemonServer::bind(client).await?.run().await
}

pub async fn monitor_processes(tracer_client: &mut TracerClient) -> Result<()> {
    tracer_client.poll_process_metrics().await?;
    tracer_client.refresh_sysinfo().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::client::exporters::log_forward::LogForward;
    use crate::client::exporters::log_writer::LogWriterEnum;
    use crate::client::TracerClient;
    use crate::common::types::cli::interactive::InteractiveInitArgs;
    use crate::common::types::cli::params::TracerCliInitArgs;
    use crate::config::Config;
    use crate::daemon::daemon_run::monitor_processes;
    use dotenv::dotenv;

    fn load_test_config() -> Config {
        Config::default()
    }

    pub fn setup_env_vars(region: &str) {
        dotenv().ok(); // Load from .env file in development
        std::env::set_var("AWS_REGION", region);
    }

    #[tokio::test]
    async fn test_monitor_processes() -> Result<(), anyhow::Error> {
        let config = load_test_config();
        let region = "us-east-2";

        setup_env_vars(region);

        // let aurora_client: dyn LogWriter = AuroraClient::try_new(&config, None).await.unwrap();

        let log_forward_client = LogWriterEnum::Forward(
            LogForward::try_new(&config.log_forward_endpoint_dev.clone().unwrap())
                .await
                .expect("Failed to create LogForward"),
        );

        let default_args = InteractiveInitArgs::from_partial(TracerCliInitArgs {
            pipeline_name: Some("test-pipeline".into()),
            ..Default::default()
        })
        .into_cli_args();

        let mut tracer_client = TracerClient::new(config, log_forward_client, default_args)
            .await?;
        let result = monitor_processes(&mut tracer_client).await;
        if result.is_ok() {
            Ok(result?)
        } else {
            Err(result.unwrap_err())
        }
    }
}
