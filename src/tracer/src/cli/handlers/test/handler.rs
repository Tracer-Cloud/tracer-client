use crate::cli::handlers::info;
use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::handlers::terminate;
use crate::cli::handlers::test::arguments::TracerCliTestArgs;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::cli::handlers::test::requests::{get_user_id_from_daemon, update_run_name_for_test};
use crate::utils::user_id_resolution::extract_user_id;

use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::info_message;
use crate::utils::system_info::check_sudo_with_procfs_option;
use crate::utils::workdir::TRACER_WORK_DIR;

use anyhow::Result;
use colored::Colorize;

/// TODO: fastquorum segfault on ARM mac; Rosetta/x86 pixi option may be needed.
pub async fn test(args: TracerCliTestArgs, config: Config, api_client: DaemonClient) -> Result<()> {
    // this is the entry function for the test command
    check_sudo_with_procfs_option("test", args.init_args.force_procfs);

    // Resolve the pipeline early so we can pass it to both functions
    let (init_args, selected_test_pipeline) = args.resolve_test_arguments()?;
    let daemon_was_already_running = DaemonServer::is_running();

    if daemon_was_already_running {
        run_test_with_existing_daemon(&api_client, selected_test_pipeline).await
    } else {
        run_test_with_new_daemon(init_args, config, &api_client, selected_test_pipeline).await
    }
}

/// Initialize daemon with new pipeline name and run test pipeline
async fn run_test_with_new_daemon(
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

/// Run test pipeline when daemon is already running
async fn run_test_with_existing_daemon(
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
