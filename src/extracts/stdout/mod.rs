use std::{path::Path, sync::Arc};

use crate::{
    tracer_client::LinesBufferArc,
    utils::{debug_log::Logger, http_client::send_http_body},
};
use anyhow::Result;
use linemux::MuxedLines;
use serde_json::json;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;

// Todo: A lot of code is duplicated between this file and syslog. Maybe we could extract the file reading code into a separate module?
pub struct StdoutWatcher {}

pub async fn run_stdout_lines_read_thread(
    stdout_file_path: &str,
    stderr_file_path: &str,
    pending_stdout_stderr_lines: (LinesBufferArc, LinesBufferArc),
) {
    let line_reader = MuxedLines::new();

    if line_reader.is_err() {
        return;
    }

    let mut line_reader = line_reader.unwrap();

    let stdout_file_path = Path::new(stdout_file_path);
    let stderr_file_path = Path::new(stderr_file_path);

    let result = line_reader.add_file(stdout_file_path).await;

    if result.is_err() {
        return;
    }

    let result = line_reader.add_file(stderr_file_path).await;
    if result.is_err() {
        return;
    }

    let (pending_stdout_lines, pending_stderr_lines) = pending_stdout_stderr_lines;

    while let Ok(Some(line)) = line_reader.try_next().await {
        if line.source() == stdout_file_path {
            let line = line.line();
            let mut vec = pending_stdout_lines.write().await;
            vec.push(line.to_string());
        } else if line.source() == stderr_file_path {
            let line = line.line();
            let mut vec = pending_stderr_lines.write().await;
            vec.push(line.to_string());
        }
    }
}

impl Default for StdoutWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl StdoutWatcher {
    pub fn new() -> StdoutWatcher {
        StdoutWatcher {}
    }

    pub async fn poll_stdout(
        &mut self,
        service_url: &str,
        api_key: &str,
        pending_lines: Arc<RwLock<Vec<String>>>,
        is_error: bool,
    ) -> Result<()> {
        let logger = Logger::new();

        if pending_lines.read().await.is_empty() {
            // logger.into("No lines from stdout to send", None).await;
            return Ok(());
        }

        let url = format!("{}/stdout-capture", service_url);

        let body = json!({
            "lines": *pending_lines.as_ref().read().await,
            "isError": is_error
        });

        logger
            .log(&format!("Sending stdout lines: {:?}", body), None)
            .await;

        pending_lines.write().await.clear();

        send_http_body(&url, api_key, &body).await?;

        Ok(())
    }
}
