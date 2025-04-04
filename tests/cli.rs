use rand;
use sqlx::PgPool;
use std::sync::Arc;
use tempfile::TempDir;
use tracer::config_manager::Config;
use tracer::exporters::db::AuroraClient;
use tracer::tracer_client::TracerClient;
use tracer::types::aws::aws_region::AwsRegion;
use tracer::types::cli::TracerCliInitArgs;
use tracer::types::config::AwsConfig;

async fn setup_client(pool: PgPool) -> Result<TracerClient, anyhow::Error> {
    // todo: generate a random port when spawning a test server
    // this is non atomic and can be improved in many ways
    let rand_port = rand::random::<u16>();
    // if already used, skip...

    let server_address = format!("127.0.0.1:{}", rand_port);

    let config = Config {
        api_key: "test_key".to_string(),
        process_polling_interval_ms: 10000000,
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

    let dir = TempDir::new()?;

    let db_client = Arc::new(AuroraClient::from_pool(pool));
    // todo(ENG-238): remove arc, as it's already an arc inside

    let args = TracerCliInitArgs::default();

    TracerClient::new(
        config,
        dir.path().to_str().unwrap().to_string(),
        db_client,
        args,
    )
    .await
}

#[sqlx::test]
async fn info(pool: PgPool) {
    setup_client(pool).await.unwrap();
}
