// src/cli.rs
use crate::extracts::process_watcher::ShortLivedProcessLog;
use crate::utils::debug_log::Logger;
use anyhow::{Context, Result};
use http::StatusCode;
use serde::Deserialize;
use serde_json::{from_str, json};
use std::path::PathBuf;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};
use tokio_stream::StreamExt;

use super::structs::{InfoResponse, Message, RunData, TagData, UploadData};

pub struct APIClient {
    base_uri: String,
    client: reqwest::Client,
}

impl APIClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_uri: base_url,
            client: reqwest::Client::new(), // todo: timeout, max payload?
        }
    }

    pub fn get_url(&self, path: &str) -> String {
        format!("{}{}", self.base_uri, path)
    }

    pub async fn send_log_request(&self, payload: Message) -> Result<()> {
        self.client
            .post(self.get_url("/logs"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_alert_request(&self, payload: Message) -> Result<()> {
        self.client
            .post(self.get_url("/alert"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_start_run_request(&self) -> Result<Option<RunData>> {
        let data: Option<RunData> = self
            .client
            .post(self.get_url("/start"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(data)
    }

    pub async fn send_terminate_request(&self) -> Result<()> {
        self.client
            .post(self.get_url("/terminate"))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_end_request(&self) -> Result<()> {
        self.client
            .post(self.get_url("/end"))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_info_request(&self) -> Result<InfoResponse> {
        let data: InfoResponse = self
            .client
            .get(self.get_url("/start"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(data)
    }

    pub async fn send_refresh_config_request(&self) -> Result<()> {
        self.client
            .post(self.get_url("/refresh-config"))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_update_tags_request(&self, payload: TagData) -> Result<()> {
        self.client
            .post(self.get_url("/tag"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_log_short_lived_process_request(
        &self,
        payload: ShortLivedProcessLog,
    ) -> Result<()> {
        self.client
            .put(self.get_url("/log-short-lived-process"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_upload_file_request(&self, payload: UploadData) -> Result<()> {
        self.client
            .put(self.get_url("/upload"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
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
