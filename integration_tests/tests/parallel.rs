use bollard::Docker;
use sqlx::PgPool;

mod common;

#[tokio::test]
#[ignore = "Integrations Test runs Seperately"]
async fn test_parallel_mode_works() {
    let db_profile = "db";

    //  Step 1: Start *only* the database
    common::start_docker_compose(db_profile).await;

    //  Step 2: Wait and run migrations
    let db_url = "postgres://postgres:postgres@localhost:5432/tracer_db";
    let pool = common::setup_db(db_url).await;

    // Step 3: Now start your test containers
    let container_name = "parallel_tests";
    common::start_docker_compose(container_name).await;

    // Step 4: Monitor test containers as usual
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

    let run_name = "parallel-tag";
    let _ = log_handle.await;

    query_and_assert_parallel_mode(&pool, run_name).await;

    // Tear everything down at the end
    common::end_docker_compose(container_name).await;
    common::end_docker_compose(db_profile).await;
}

async fn query_and_assert_parallel_mode(pool: &PgPool, run_name: &str) {
    let tools_tracked: Vec<(i64,)> = sqlx::query_as(
        r#"
            SELECT COUNT(DISTINCT resource_attributes->>'system_properties.hostname') AS unique_hosts
            FROM batch_jobs_logs
            WHERE run_name = $1;
        "#,
    )
    .bind(run_name)
    .fetch_all(pool)
    .await
    .expect("failed ");

    assert_eq!(tools_tracked.len(), 1);

    let unique_hosts = tools_tracked.first().unwrap().0;

    assert_eq!(unique_hosts, 2)
}
