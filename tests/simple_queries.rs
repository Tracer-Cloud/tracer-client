use bollard::Docker;
use sqlx::PgPool;

mod common;

#[tokio::test]
async fn test_simple_queries_works() {
    let container_name = "integrations_tests";

    // Step 1: Start Docker Compose to run the container
    common::start_docker_compose(container_name).await;

    // Step 2: Monitor the container and wait for it to finish

    let docker = Docker::connect_with_local_defaults().expect("Failed to connect to Docker");

    common::monitor_container(&docker, container_name).await;

    // Step 3: Query the database and make assertions
    let pool = PgPool::connect("postgres://postgres:postgres@localhost:5432/tracer_db")
        .await
        .unwrap();

    let job_id = "test-tag";

    query_and_assert_tool_tracked(&pool, job_id).await;

    query_datasets_processed(&pool, job_id).await;

    common::end_docker_compose(container_name).await;
}

async fn query_and_assert_tool_tracked(pool: &PgPool, job_id: &str) {
    let tools_tracked: Vec<(String,)> = sqlx::query_as(
        r#"
            SELECT DISTINCT(data->'attributes'->'Process'->>'tool_name') AS tool_name
            FROM batch_jobs_logs
            WHERE 
            job_id = $1
            AND
            data->'attributes'->'Process'->>'tool_name' IS NOT NULL;
        "#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .expect("failed ");
    assert!(!tools_tracked.is_empty());

    let flat_tools: Vec<String> = tools_tracked.into_iter().map(|v| v.0).collect();

    assert!(flat_tools.contains(&("python3".to_string())))
}

async fn query_datasets_processed(pool: &PgPool, job_id: &str) {
    let tools_tracked: Vec<(String, i64)> = sqlx::query_as(
        r#"
            SELECT 
                data->>'process_status' AS process_status,
                MAX((data->'attributes'->'ProcessDatasetStats'->>'total')::BIGINT) AS total_samples
            FROM batch_jobs_logs
            WHERE data->>'process_status' = 'datasets_in_process'
            AND data->>'run_name' = $1
            GROUP BY process_status;
        "#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .expect("failed ");
    assert_eq!(tools_tracked.len(), 1);

    let total_samples = tools_tracked.first().unwrap().1;

    assert_eq!(total_samples, 3)
}
