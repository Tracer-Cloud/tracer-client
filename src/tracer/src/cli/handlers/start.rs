use crate::cli::handlers::info::InfoDisplay;
use crate::daemon::client::DaemonClient;
use crate::{error_message, success_message};
use colored::Colorize;

pub async fn start(api_client: &DaemonClient, json: bool) {
    let started = match api_client.send_start_request().await {
        Ok(started) => started,
        Err(_) => {
            return;
        }
    };
    if let Some(pipeline_data) = started {
        success_message!("Run started successfully.");
        let display = InfoDisplay::new(150, json);
        display.print(pipeline_data);
    } else {
        error_message!(
            "Cannot start a new run while another is active. Please stop the current run first."
        );
    }
}
