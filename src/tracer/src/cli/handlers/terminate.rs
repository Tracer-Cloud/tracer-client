use crate::daemon::client::DaemonClient;
use crate::process_identification::constants::PID_FILE;
use crate::{error_message, success_message};
use colored::Colorize;
use std::fs;

fn get_pid() -> Option<String> {
    let contents = fs::read_to_string(PID_FILE).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
pub async fn terminate(api_client: &DaemonClient) -> bool{
    if let Err(e) = api_client.send_terminate_request().await {
        error_message!("Failed to send terminate request to the daemon: {e}");
        error_message!(
            "Try running `sudo kill -9 {}` to forcefully terminate the daemon.",
            get_pid().unwrap_or_else(|| "unknown PID".to_string())
        );
        return false;
    }
    success_message!("Daemon server terminated successfully.");
    true
}
