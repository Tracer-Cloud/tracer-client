use assert_cmd::assert::Assert;
use assert_cmd::Command;
use sqlx::PgPool;
use std::net::SocketAddr;
use tempfile::TempDir;
use tokio::task::JoinHandle;
use tracer::config_manager::Config;
use tracer::daemon_communication::server::DaemonServer;
use tracer::exporters::db::AuroraClient;
use tracer::tracer_client::TracerClient;
use tracer::types::aws::aws_region::AwsRegion;
use tracer::types::cli::TracerCliInitArgs;
use tracer::types::config::AwsConfig;

pub struct TestServer {
    dir: TempDir,
    handle: JoinHandle<anyhow::Result<()>>,
    addr: SocketAddr,
}

impl TestServer {
    async fn setup_client(
        pool: PgPool,
        server: String,
        path: String,
    ) -> Result<TracerClient, anyhow::Error> {
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
            database_secrets_arn: "".to_string(),
            database_host: "should-not-be-used".to_string(), // cuz we have pg pool
            database_name: "should-not-be-used".to_string(),
            // todo: sqlite / postgres in transaction mode / many schemas
            grafana_workspace_url: "".to_string(),
            server,
        };

        let db_client = AuroraClient::from_pool(pool);

        let args = TracerCliInitArgs::default();

        TracerClient::new(config, path, db_client, args).await
    }

    async fn get_tracer(pool: PgPool, path: String) -> Result<DaemonServer, anyhow::Error> {
        let server: SocketAddr = "127.0.0.1:0".parse()?; // 0: means port will be picked by the OS
        let client = Self::setup_client(pool, server.to_string(), path).await?;

        let server = DaemonServer::bind(client, server).await?;
        Ok(server)
    }

    pub async fn send_command(&self, command: &[&str]) -> Assert {
        let mut cmd = Command::cargo_bin("tracer").unwrap();
        cmd.env("TRACER_SERVER", self.addr.to_string());
        cmd.env("RUST_BACKTRACE", "1");
        cmd.args(command);
        cmd.timeout(std::time::Duration::from_secs(30));

        tokio::task::spawn_blocking(move || cmd.assert())
            .await
            .unwrap()
    }

    pub async fn launch(pool: PgPool) -> anyhow::Result<Self> {
        let dir = TempDir::new()?;
        let server = Self::get_tracer(pool, dir.path().to_str().unwrap().to_string()).await?;

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
