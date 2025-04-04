use assert_cmd::prelude::*;
use predicates::str::contains;
use sqlx::PgPool;
mod common;

#[sqlx::test]
async fn info(pool: PgPool) {
    let server = common::test_server::TestServer::launch(pool).await.unwrap();

    server
        .send_command(&["info"])
        .await
        .success()
        .stdout(contains("Daemon status: Running"))
        .stdout(contains("Total Run Time"));

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}
