use crate::cli::handlers::info;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::cli::handlers::test::requests::{get_user_id_from_daemon, update_run_name_for_test};

use crate::daemon::client::DaemonClient;
use crate::info_message;

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
