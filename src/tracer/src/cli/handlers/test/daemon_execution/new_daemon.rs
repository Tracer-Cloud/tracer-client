use crate::cli::handlers::info;
use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::handlers::terminate;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::utils::user_id_resolution::extract_user_id;

use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::info_message;
use crate::utils::workdir::TRACER_WORK_DIR;

use anyhow::Result;
use colored::Colorize;

/// Initialize daemon with new pipeline name and run test pipeline
pub async fn run_test_with_new_daemon(
    mut init_args: TracerCliInitArgs,
    config: Config,
    api_client: &DaemonClient,
    selected_test_pipeline: Pipeline,
) -> Result<()> {
    info_message!("[run_test_with_new_daemon] Daemon is not running, starting new instance...");
    TRACER_WORK_DIR.init().expect("creating work files failed");

    // Configure init args for test scenarios
    init_args.configure_for_test();

    // Set the pipeline name only if user hasn't provided one
    if init_args.pipeline_name.is_none() {
        // Extract user_id with comprehensive fallback strategies and Sentry instrumentation
        let user_id = extract_user_id(init_args.tags.user_id.clone())
            .unwrap_or_else(|_| "unknown".to_string());

        let new_test_pipeline_name = format!("test-{}-{}", selected_test_pipeline.name(), user_id);
        init_args.pipeline_name = Some(new_test_pipeline_name);
    }

    crate::cli::handlers::init::init(init_args, config, api_client).await?;

    // Run the pipeline after the daemon has been started
    let result = selected_test_pipeline.execute();

    // Show info to check if the process where recognized correctly s
    info::info(api_client, false).await;

    info_message!("Shutting down daemon following test completion...");
    terminate::terminate(api_client).await;

    result
}
