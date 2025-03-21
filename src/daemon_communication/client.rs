// src/cli.rs
use anyhow::Result;
use serde::Deserialize;
use serde_json::{from_str, json};
use std::path::PathBuf;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

use crate::extracts::process_watcher::ShortLivedProcessLog;
use crate::utils::debug_log::Logger;

use super::structs::InfoResponse;

pub async fn send_log_request(socket_path: &str, message: String) -> Result<()> {
    let mut socket = UnixStream::connect(socket_path).await?;

    let log_request = json!({
            "command": "log",
            "message": message
    });
    let log_request_json =
        serde_json::to_string(&log_request).expect("Failed to serialize log request");
    socket.write_all(log_request_json.as_bytes()).await?;

    Ok(())
}

pub async fn send_alert_request(socket_path: &str, message: String) -> Result<()> {
    let mut socket = UnixStream::connect(socket_path).await?;
    let alert_request: serde_json::Value = json!({
            "command": "alert",
            "message": message
    });
    let alert_request_json =
        serde_json::to_string(&alert_request).expect("Failed to serialize alrt request");
    socket.write_all(alert_request_json.as_bytes()).await?;

    Ok(())
}

pub async fn send_terminate_request(socket_path: &str) -> Result<()> {
    let mut socket = UnixStream::connect(socket_path).await?;

    let terminate_request = json!({
            "command": "terminate"
    });

    let terminate_request_json =
        serde_json::to_string(&terminate_request).expect("Failed to serialize terminate request");

    socket.write_all(terminate_request_json.as_bytes()).await?;

    Ok(())
}

pub async fn send_start_run_request(socket_path: &str) -> Result<()> {
    let mut socket = UnixStream::connect(socket_path).await?;

    let start_request = json!({
            "command": "start"
    });

    let start_request_json =
        serde_json::to_string(&start_request).expect("Failed to serialize start request");

    socket.write_all(start_request_json.as_bytes()).await?;

    socket.shutdown().await?;

    #[derive(Deserialize)]
    struct StartRunResponse {
        run_name: String,
    }

    let mut buffer = [0; 1024];
    let n = socket.read(&mut buffer).await?;
    let response = std::str::from_utf8(&buffer[..n])?;
    let response: StartRunResponse = from_str(response)?;

    println!("Started a new run with name: {}", response.run_name);

    Ok(())
}

pub async fn send_end_run_request(socket_path: &str) -> Result<()> {
    let mut socket = UnixStream::connect(socket_path).await?;

    let end_request = json!({
            "command": "end"
    });

    let end_request_json =
        serde_json::to_string(&end_request).expect("Failed to serialize start request");

    socket.write_all(end_request_json.as_bytes()).await?;

    Ok(())
}

pub async fn send_info_request(socket_path: &str) -> Result<InfoResponse> {
    let mut socket = UnixStream::connect(socket_path).await?;

    let ping_request = json!({
            "command": "info"
    });

    let info_request_json =
        serde_json::to_string(&ping_request).expect("Failed to serialize info request");

    socket.write_all(info_request_json.as_bytes()).await?;

    socket.shutdown().await?;

    let mut buffer = [0; 1024];
    let n = socket.read(&mut buffer).await?;
    let response = std::str::from_utf8(&buffer[..n])?;
    let response: InfoResponse = from_str(response)?;

    Ok(response)
}

pub async fn send_refresh_config_request(socket_path: &str) -> Result<()> {
    let mut socket = UnixStream::connect(socket_path).await?;

    let setup_request = json!({
            "command": "refresh_config"
    });

    let setup_request_json =
        serde_json::to_string(&setup_request).expect("Failed to serialize setup request");

    socket.write_all(setup_request_json.as_bytes()).await?;

    Ok(())
}

pub async fn send_update_tags_request(socket_path: &str, tags: &Vec<String>) -> Result<()> {
    let mut socket = UnixStream::connect(socket_path).await?;

    let tag_request = json!({
            "command": "tag",
            "tags": tags
    });

    let tag_request_json =
        serde_json::to_string(&tag_request).expect("Failed to serialize tag request");

    socket.write_all(tag_request_json.as_bytes()).await?;

    Ok(())
}

pub async fn send_log_short_lived_process_request(
    socket_path: &str,
    log: ShortLivedProcessLog,
) -> Result<()> {
    let mut socket = UnixStream::connect(socket_path).await?;

    let log_request = json!({
            "command": "log_short_lived_process",
            "log": log
    });

    let log_request_json =
        serde_json::to_string(&log_request).expect("Failed to serialize log request");

    socket.write_all(log_request_json.as_bytes()).await?;

    Ok(())
}

pub async fn send_upload_file_request(socket_path: &str, file_path: &PathBuf) -> Result<()> {
    let logger = Logger::new();
    logger
        .log(
            "send_upload_file_request",
            Some(&json!({
                "file_path": file_path,
                "socket_path": &socket_path

            })),
        )
        .await;

    let mut socket = UnixStream::connect(socket_path).await?;

    let upload_request = json!({
        "command": "upload",
        "file_path": file_path
    });

    let upload_request_json =
        serde_json::to_string(&upload_request).expect("Failed to serialize upload request");

    socket.write_all(upload_request_json.as_bytes()).await?;

    logger
        .log(
            "send_upload_file_request//socket.write_all",
            Some(&upload_request),
        )
        .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::AsyncReadExt, net::UnixListener};

    fn create_tmp_socket_path() -> (tempfile::TempDir, String) {
        let tempdir = tempfile::tempdir().expect("Failed to create temp dir");
        std::fs::create_dir_all(tempdir.path()).expect("Failed to create temp dirs");
        let file_path = tempdir.path().join("tracerd.sock");

        std::fs::File::create(&file_path).expect("failed to create file");

        (tempdir, file_path.to_str().unwrap().to_string())
    }

    fn setup_test_unix_listener(socket_path: &str) -> UnixListener {
        let _ = env_logger::builder().is_test(true).try_init();
        if std::fs::metadata(socket_path).is_ok() {
            std::fs::remove_file(socket_path).expect("Failed to remove existing socket file");
        }

        UnixListener::bind(socket_path).expect("Failed to bind to unix socket")
    }

    async fn check_listener_value(listener: &UnixListener, expected_value: &str) {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = [0; 1024];
        let n = stream.read(&mut buffer).await.unwrap();
        let received = std::str::from_utf8(&buffer[..n]).unwrap();
        assert_eq!(received, expected_value);
    }

    #[tokio::test]
    async fn test_send_log_request() -> Result<()> {
        let (_temp, socket_path) = create_tmp_socket_path();
        let listener = setup_test_unix_listener(&socket_path);
        let message = "Test Message".to_string();

        send_log_request(&socket_path, message.clone()).await?;

        check_listener_value(
            &listener,
            json!({
                "command": "log",
                "message": message
            })
            .to_string()
            .as_str(),
        )
        .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_send_alert_request() -> Result<()> {
        let (_temp, socket_path) = create_tmp_socket_path();
        let listener = setup_test_unix_listener(&socket_path);
        let message = "Test Message".to_string();

        send_alert_request(&socket_path, message.clone()).await?;

        check_listener_value(
            &listener,
            json!({
                "command": "alert",
                "message": message
            })
            .to_string()
            .as_str(),
        )
        .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_send_terminate_request() -> Result<()> {
        let (_temp, socket_path) = create_tmp_socket_path();
        let listener = setup_test_unix_listener(&socket_path);

        send_terminate_request(&socket_path).await?;

        check_listener_value(
            &listener,
            json!({
                "command": "terminate"
            })
            .to_string()
            .as_str(),
        )
        .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_send_end_run_request() -> Result<()> {
        let (_temp, socket_path) = create_tmp_socket_path();
        let listener = setup_test_unix_listener(&socket_path);

        send_end_run_request(&socket_path).await?;

        check_listener_value(
            &listener,
            json!({
                "command": "end"
            })
            .to_string()
            .as_str(),
        )
        .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_send_refresh_config_request() -> Result<()> {
        let (_temp, socket_path) = create_tmp_socket_path();
        let listener = setup_test_unix_listener(&socket_path);

        send_refresh_config_request(&socket_path).await?;

        check_listener_value(
            &listener,
            json!({
                "command": "refresh_config"
            })
            .to_string()
            .as_str(),
        )
        .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_send_update_tags_request() -> Result<()> {
        let (_temp, socket_path) = create_tmp_socket_path();
        let listener = setup_test_unix_listener(&socket_path);
        let tags = vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()];

        send_update_tags_request(&socket_path, &tags).await?;

        check_listener_value(
            &listener,
            json!({
                "command": "tag",
                "tags": tags
            })
            .to_string()
            .as_str(),
        )
        .await;

        Ok(())
    }

    #[tokio::test]
    async fn test_send_upload_file_request() -> Result<()> {
        let (_temp, socket_path) = create_tmp_socket_path();
        let listener = setup_test_unix_listener(&socket_path);
        let file_path = PathBuf::from("log_outgoing_http_calls.txt".to_string());

        send_upload_file_request(&socket_path, &file_path).await?;

        check_listener_value(
            &listener,
            json!({
                "command": "upload",
                "file_path": file_path.clone()
            })
            .to_string()
            .as_str(),
        )
        .await;

        Ok(())
    }
}
