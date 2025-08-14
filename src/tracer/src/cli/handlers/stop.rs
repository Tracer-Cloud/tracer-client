use crate::daemon::client::DaemonClient;
use crate::{error_message, success_message};
use colored::Colorize;

pub async fn stop(api_client: &DaemonClient) {
    let stopped = match api_client.send_stop_request().await {
        Ok(stopped) => stopped,
        Err(_) => {
            return;
        }
    };
    if stopped {
        success_message!("Run stopped successfully.");
    } else {
        error_message!("No run is currently active.");
    }
}
