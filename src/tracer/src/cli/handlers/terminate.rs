use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::{error_message, info_message, success_message, warning_message};
use colored::Colorize;
use std::fs;

pub(super) fn get_pid() -> Option<String> {
    let contents = fs::read_to_string(&TRACER_WORK_DIR.pid_file).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub async fn terminate(api_client: &DaemonClient) -> bool {
    if !DaemonServer::is_running() {
        warning_message!("Daemon server is not running. Nothing to terminate.");
        return false;
    }
    if let Err(e) = api_client.send_terminate_request().await {
        error_message!("Failed to send terminate request to the daemon: {e}");
        error_message!(
            "Try running `sudo kill -9 {}` to forcefully terminate the daemon.",
            get_pid().unwrap_or_else(|| "unknown PID".to_string())
        );
        return false;
    }
    if !check_port_conflict().await {
        error_message!(
            "Port conflict detected. Please wait up to a minute for the port to be released."
        );
        return false;
    }
    success_message!("Daemon server terminated successfully.");
    true
}

async fn check_port_conflict() -> bool {
    // Add retry mechanism with delays to ensure port is released
    const MAX_RETRIES: u32 = 60;
    const RETRY_DELAY_MS: u64 = 1000;
    let port = DEFAULT_DAEMON_PORT;
    info_message!(
        "Checking if port {} is available... (may take up to 1 minute)",
        port
    );
    for attempt in 1..=MAX_RETRIES {
        info_message!(
            "Waiting for port to be released (attempt {}/{})...",
            attempt,
            MAX_RETRIES
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;

        if std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
            return true;
        }
    }

    false
}
