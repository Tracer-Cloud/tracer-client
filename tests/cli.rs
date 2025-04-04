use assert_cmd::prelude::*; // Add methods on commands
use assert_cmd::Command;
use predicates::prelude::*; // Used for writing assertions
                            // use rand;
use assert_cmd::assert::Assert;
use clap::builder::Str;
use predicates::str::contains;
use sqlx::PgPool;
use sqlx::__rt::JoinHandle;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;
use tokio_stream::StreamExt;
use tracer::config_manager::Config;
use tracer::exporters::db::AuroraClient;
use tracer::tracer_client::TracerClient;
use tracer::types::aws::aws_region::AwsRegion;
use tracer::types::cli::TracerCliInitArgs;
use tracer::types::config::AwsConfig;

async fn setup_client(
    pool: PgPool,
    server_address: String,
    path: String,
) -> Result<TracerClient, anyhow::Error> {
    let config = Config {
        api_key: "test_key".to_string(),
        process_polling_interval_ms: 100,
        batch_submission_interval_ms: 10000000,
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
        server_address,
    };

    let db_client = Arc::new(AuroraClient::from_pool(pool));
    // todo(ENG-238): remove arc, as it's already an arc inside

    let args = TracerCliInitArgs::default();

    TracerClient::new(config, path, db_client, args).await
}

async fn get_tracer(pool: PgPool, path: String) -> Result<(TracerClient, String), anyhow::Error> {
    // todo: generate a random port when spawning a test server
    // this is non atomic and can be improved in many ways
    let rand_port = rand::random::<u16>();
    // if already used, skip...

    let server_address = format!("127.0.0.1:{}", rand_port);

    Ok((
        setup_client(pool, server_address.clone(), path).await?,
        server_address,
    ))
}

async fn send_command(addr: &str, command: &[&str]) -> Assert {
    let mut cmd = Command::cargo_bin("tracer").unwrap();
    cmd.env("TRACER_SERVER_ADDRESS", addr);
    cmd.args(command);
    cmd.timeout(std::time::Duration::from_secs(5));

    tokio::task::spawn_blocking(move || cmd.assert())
        .await
        .unwrap()
}

#[sqlx::test]
async fn info(pool: PgPool) {
    // todo: move all harness into a new macros
    let dir = TempDir::new().unwrap();
    let (tracer, addr) = get_tracer(pool, dir.path().to_str().unwrap().to_string())
        .await
        .unwrap();
    let handle = tokio::task::spawn(tracer.run());
    // todo: do health check N times?

    send_command(addr.as_str(), &["info"])
        .await
        .success()
        .stdout(contains("Daemon status: Running"))
        .stdout(contains("Total Run Time"));

    send_command(addr.as_str(), &["terminate"]).await.success();

    handle.await.unwrap().unwrap();
    drop(dir); // to ensure dir is still in scope
}
