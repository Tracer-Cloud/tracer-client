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

#[sqlx::test]
async fn log(pool: PgPool) {
    let server = common::test_server::TestServer::launch(pool).await.unwrap();

    server
        .send_command(&["log", "some_message"])
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}

#[sqlx::test]
async fn alert(pool: PgPool) {
    let server = common::test_server::TestServer::launch(pool).await.unwrap();

    server
        .send_command(&["alert", "some_message"])
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}

#[sqlx::test]
async fn end(pool: PgPool) {
    let server = common::test_server::TestServer::launch(pool).await.unwrap();

    server
        .send_command(&["end"])
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}

#[sqlx::test]
async fn tag(pool: PgPool) {
    let server = common::test_server::TestServer::launch(pool).await.unwrap();

    server
        .send_command(&["tag", "tag1", "tag2"])
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}

#[sqlx::test]
async fn upload(pool: PgPool) {
    let server = common::test_server::TestServer::launch(pool).await.unwrap();

    server
        .send_command(&["upload", "/Users/blaginin/jbr_err_pid1039.log"]) // random file
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}
