use predicates::str::contains;
use sqlx::PgPool;
mod common;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[tokio::test]
async fn info() {
    let server = common::test_server::TestServer::launch().await.unwrap();

    server
        .send_command(&["info"])
        .await
        .success()
        .stdout(contains("Running")) // Daemon status
        .stdout(contains("Total Run Time"));

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}

#[tokio::test]
async fn log() {
    let server = common::test_server::TestServer::launch().await.unwrap();

    server
        .send_command(&["log", "some_message"])
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}

#[tokio::test]
async fn alert() {
    let server = common::test_server::TestServer::launch().await.unwrap();

    server
        .send_command(&["alert", "some_message"])
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}

#[tokio::test]
async fn end() {
    let server = common::test_server::TestServer::launch().await.unwrap();

    server
        .send_command(&["end"])
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}

#[tokio::test]
async fn tag() {
    let server = common::test_server::TestServer::launch().await.unwrap();

    server
        .send_command(&["tag", "tag1", "tag2"])
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}

#[tokio::test]
async fn upload() {
    let server = common::test_server::TestServer::launch().await.unwrap();

    server
        .send_command(&["upload", "/Users/blaginin/jbr_err_pid1039.log"]) // random file
        .await
        .success()
        .stdout(contains("Command sent successfully"));

    // todo: also check tracer.logs?

    server.send_command(&["terminate"]).await.success();
    server.finished().await.unwrap()
}
