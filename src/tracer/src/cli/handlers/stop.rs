use crate::daemon::client::DaemonClient;
use crate::{error_message, success_message};
use colored::Colorize;


pub async fn stop(api_client: &DaemonClient) -> bool {
    if let Err(e) = api_client.send_start_request().await {
        error_message!("Failed to send a start request to the daemon: {e}");
        error_message!(
            "Try running `tracer` to forcefully terminate the daemon.",
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

