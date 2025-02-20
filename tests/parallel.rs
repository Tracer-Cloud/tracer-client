use bollard::Docker;
use sqlx::PgPool;

mod common;

#[tokio::test]
async fn test_parallel_mode_works() {
    let container_name = "parallel_tests";

    // Step 1: Start Docker Compose to run the container
    common::start_docker_compose(container_name).await;

    // Step 2: Monitor the container and wait for it to finish

    let docker = Docker::connect_with_local_defaults().expect("Failed to connect to Docker");

    common::monitor_container(&docker, container_name).await;

    // Step 3: Query the database and make assertions
    let pool = PgPool::connect("postgres://postgres:postgres@localhost:5432/tracer_db")
        .await
        .unwrap();

    let job_id = "parallel-tag";

    query_and_assert_parrallel_mode(&pool, job_id).await;

    common::end_docker_compose(container_name).await;
}

async fn query_and_assert_parrallel_mode(pool: &PgPool, job_id: &str) {
    let tools_tracked: Vec<(i64,)> = sqlx::query_as(
        r#"
            SELECT COUNT(DISTINCT data->'attributes'->'system_properties'->>'hostname') AS unique_hosts
            FROM batch_jobs_logs
            WHERE data->>'run_name' = $1;
        "#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .expect("failed ");
    assert_eq!(tools_tracked.len(), 1);

    let unique_hosts = tools_tracked.first().unwrap().0;

    assert_eq!(unique_hosts, 2)
}
