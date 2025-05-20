use assert_cmd::assert::Assert;
use assert_cmd::Command;
use sqlx::PgPool;
use std::net::SocketAddr;
use tempfile::TempDir;
use tokio::task::JoinHandle;
use tracer_aws::config::AwsConfig;
use tracer_aws::types::aws_region::AwsRegion;
use tracer_client::config_manager::Config;
use tracer_client::exporters::db::AuroraClient;
use tracer_client::exporters::log_forward::LogForward;
use tracer_client::exporters::log_writer::{LogWriter, LogWriterEnum};
use tracer_common::types::cli::interactive::InteractiveInitArgs;
use tracer_common::types::cli::params::TracerCliInitArgs;

use tracer_client::TracerClient;
use tracer_daemon::server::DaemonServer;

pub struct TestServer {
    dir: TempDir,
    handle: JoinHandle<anyhow::Result<()>>,
    addr: SocketAddr,
}

impl TestServer {
    async fn setup_client(server: String, path: String) -> Result<TracerClient, anyhow::Error> {
        let config = Config {
            api_key: "EAjg7eHtsGnP3fTURcPz1".to_string(),
            process_polling_interval_ms: 100,
            batch_submission_interval_ms: 10000000, // todo: check data in batch
            process_metrics_send_interval_ms: 10000000,
            file_size_not_changing_period_ms: 10000000,
            new_run_pause_ms: 10000000,
            targets: vec![],
            aws_init_type: AwsConfig::Env,
            aws_region: AwsRegion::Eu,
            database_secrets_arn: Some("".to_string()),
            database_host: Some("should-not-be-used".to_string()), // cuz we have pg pool
            database_name: "should-not-be-used".to_string(),
            // todo: sqlite / postgres in transaction mode / many schemas
            grafana_workspace_url: "".to_string(),
            server,
            config_sources: vec![],
            sentry_dsn: None,
            log_forward_endpoint_dev: None,
            log_forward_endpoint_prod: None,
        };

        let log_forward_endpoint = "https://sandbox.tracer.cloud/api/logs-forward/dev";

        let log_forward_client = LogWriterEnum::Forward(
            LogForward::try_new(log_forward_endpoint)
                .await
                .expect("Failed to create LogForward"),
        );

        let args = InteractiveInitArgs::from_partial(TracerCliInitArgs::default()).into_cli_args();

        TracerClient::new(config, path, log_forward_client, args).await
    }

    async fn get_tracer(path: String) -> Result<DaemonServer, anyhow::Error> {
        let server: SocketAddr = "127.0.0.1:0".parse()?; // 0: means port will be picked by the OS
        let client = Self::setup_client(server.to_string(), path).await?;

        let server = DaemonServer::bind(client, server).await?;
        Ok(server)
    }

    pub async fn send_command(&self, command: &[&str]) -> Assert {
        let mut cmd = Command::cargo_bin("tracer_cli").unwrap();
        cmd.env("TRACER_SERVER", self.addr.to_string());
        cmd.env("TRACER_CONFIG_DIR", "../../");
        cmd.env("RUST_BACKTRACE", "1");
        cmd.args(command);
        cmd.timeout(std::time::Duration::from_secs(30));

        tokio::task::spawn_blocking(move || cmd.assert())
            .await
            .unwrap()
    }

    pub async fn launch() -> anyhow::Result<Self> {
        let dir = TempDir::new()?;
        let server = Self::get_tracer(dir.path().to_str().unwrap().to_string()).await?;

        let addr = server.local_addr()?;
        println!("server listening on {}", addr);

        let handle = tokio::task::spawn(server.run());
        // todo: do health check N times?

        Ok(Self { dir, handle, addr })
    }

    pub async fn finished(self) -> anyhow::Result<()> {
        self.handle.await??;
        Ok(())
    }
}
