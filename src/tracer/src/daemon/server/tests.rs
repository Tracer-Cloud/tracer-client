#[cfg(test)]
mod tests {
    use crate::client::exporters::log_forward::LogForward;
    use crate::client::exporters::log_writer::LogWriterEnum;
    use crate::client::TracerClient;
    use crate::common::types::cli::interactive::InteractiveInitArgs;
    use crate::common::types::cli::params::TracerCliInitArgs;
    use crate::config::Config;
    use crate::daemon::server::process_monitor::monitor_processes;
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

        let mut tracer_client = TracerClient::new(config, log_forward_client, default_args).await?;
        let result = monitor_processes(&mut tracer_client).await;
        if result.is_ok() {
            Ok(result?)
        } else {
            Err(result.unwrap_err())
        }
    }
}
