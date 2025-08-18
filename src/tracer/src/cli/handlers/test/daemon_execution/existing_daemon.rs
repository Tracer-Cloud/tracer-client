use crate::cli::handlers::info;
use crate::cli::handlers::test::pipeline::Pipeline;

use crate::daemon::client::DaemonClient;
use crate::{info_message, warning_message};

use anyhow::Result;
use colored::Colorize;

/// Run test pipeline when daemon is already running
pub async fn run_test_with_existing_daemon(
    api_client: &DaemonClient,
    selected_test_pipeline: Pipeline,
) -> Result<()> {
    info_message!(
        "Daemon is already running, executing {} pipeline...",
        selected_test_pipeline.name()
    );

    let user_id = get_user_id_from_daemon(api_client).await;
    update_run_name_for_test(api_client, &user_id).await;

    let result = selected_test_pipeline.execute();

    // Show info to check if the process where recognized correctly s
    info::info(api_client, false).await;

    result
}

/// Get user ID from daemon with fallback to 'unknown'
pub async fn get_user_id_from_daemon(api_client: &DaemonClient) -> String {
    match api_client.send_get_user_id_request().await {
        Ok(response) if response.success => response.user_id.unwrap_or_else(|| {
            warning_message!("User ID was successful but empty, using 'unknown_user_id'");
            "unknown_user_id".to_string()
        }),
        Ok(response) => {
            warning_message!(
                "Failed to get user ID: {}, using 'unknown_user_id'",
                response.message
            );
            "unknown_user_id".to_string()
        }
        Err(e) => {
            warning_message!("Error getting user ID: {}, using 'unknown_user_id'", e);
            "unknown_user_id".to_string()
        }
    }
}

/// Update run name for test with user ID
pub async fn update_run_name_for_test(api_client: &DaemonClient, user_id: &str) {
    let new_run_name = format!("test-fastquorum-{}", user_id);
    info_message!("Updating run name to: {}", new_run_name);

    match api_client.send_update_run_name_request(new_run_name).await {
        Ok(response) if response.success => {
            info_message!("Run name updated successfully: {}", response.message);
        }
        Ok(response) => {
            warning_message!("Failed to update run name: {}", response.message);
        }
        Err(e) => {
            warning_message!("Error updating run name: {}", e);
        }
    }
}
