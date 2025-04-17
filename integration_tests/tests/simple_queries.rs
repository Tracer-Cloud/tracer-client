use bollard::Docker;
use sqlx::PgPool;

mod common;

#[tokio::test]
async fn test_queries_works() {
    let container_name = "integrations_tests";

    // Step 1: Start Docker Compose to run the container
    common::start_docker_compose(container_name).await;

    // step 1b: connect and migrate on database
    let db_url = "postgres://postgres:postgres@localhost:5432/tracer_db";
    let pool = common::setup_db(db_url).await;

    // Step 2: Monitor the container and wait for it to finish
    let docker = Docker::connect_with_local_defaults().expect("Failed to connect to Docker");

    let log_handle = tokio::spawn({
        let docker = docker.clone();
        async move {
            common::print_all_container_logs(&docker).await;
            common::dump_container_file_for_all_matching(
                &docker,
                container_name,
                "/tmp/tracer/tracerd.out",
            )
            .await;
            common::dump_container_file_for_all_matching(
                &docker,
                container_name,
                "/tmp/tracer/tracerd.err",
            )
            .await;
        }
    });

    common::monitor_container(&docker, container_name).await;

    // Step 3: Query the database and make assertions
    let run_name = "test-tag";
    let _ = log_handle.await;

    query_and_assert_tool_tracked(&pool, run_name).await;

    query_datasets_processed(&pool, run_name).await;
}

async fn query_and_assert_tool_tracked(pool: &PgPool, run_name: &str) {
    let tools_tracked: Vec<(String,)> = sqlx::query_as(
        r#"
            SELECT DISTINCT(attributes->>'tool_name') AS tool_name
            FROM batch_jobs_logs
            WHERE 
            run_name = $1
            AND
            attributes ->> 'tool_name' IS NOT NULL;
        "#,
    )
    .bind(run_name)
    .fetch_all(pool)
    .await
    .expect("failed ");
    assert!(!tools_tracked.is_empty());

    let flat_tools: Vec<String> = tools_tracked.into_iter().map(|v| v.0).collect();

    assert!(flat_tools.contains(&("sim_fileopens.py".to_string())))
}

async fn query_datasets_processed(pool: &PgPool, run_name: &str) {
    let tools_tracked: Vec<(String, i64)> = sqlx::query_as(
        r#"
            SELECT 
                process_status,
                MAX((attributes ->> 'total')::BIGINT) AS total_samples
            FROM batch_jobs_logs
            WHERE process_status = 'datasets_in_process'
            AND run_name = $1
            GROUP BY process_status;
        "#,
    )
    .bind(run_name)
    .fetch_all(pool)
    .await
    .expect("failed ");
    assert_eq!(tools_tracked.len(), 1);

    let total_samples = tools_tracked.first().unwrap().1;

    assert_eq!(total_samples, 3)
}
