use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::Context;
use chrono::Utc;
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

pub struct Logger {
    log_file_path: PathBuf,
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

impl Logger {
    pub fn new() -> Self {
        Self {
            log_file_path: TRACER_WORK_DIR.debug_log.clone(),
        }
    }

    pub async fn log(&self, message: &str, context: Option<&Value>) {
        let log_message = self.format_log_message(message, context);
        self.write_to_log_file(&log_message).await
    }

    pub fn log_blocking(&self, message: &str, context: Option<&Value>) {
        let log_message = self.format_log_message(message, context);
        self.write_to_log_file_blocking(&log_message);
    }

    fn format_log_message(&self, message: &str, context: Option<&Value>) -> String {
        let timestamp = Utc::now().to_rfc3339();
        match context {
            Some(ctx) => format!(
                "[{}] {}\nContext: {}\n----------\n",
                timestamp, message, ctx
            ),
            None => format!("[{}] {}\n----------\n", timestamp, message),
        }
    }

    // TODO: why can't we maintain a persistant handle on the file?
    async fn write_to_log_file(&self, log_message: &str) {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
            .await;

        if let Err(error) = file {
            eprintln!("Failed to open log file: {}", error);
            return;
        }

        let write_result = file
            .unwrap()
            .write_all(log_message.as_bytes())
            .await
            .context("Failed to write to log file");

        if let Err(error) = write_result {
            eprintln!("Failed to write to log file: {}", error);
        }
    }

    fn write_to_log_file_blocking(&self, log_message: &str) {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path);

        if let Err(error) = file {
            eprintln!("Failed to open log file: {}", error);
            return;
        }

        let write_result = file.unwrap().write_all(log_message.as_bytes());

        if let Err(error) = write_result {
            eprintln!("Failed to write to log file: {}", error);
        }
    }
}
