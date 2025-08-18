use crate::cli::handlers::info;
use crate::cli::handlers::terminate;
use crate::cli::handlers::test::arguments::TracerCliTestArgs;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::cli::handlers::test::pipelines_git_repo::get_tracer_pipeline_path;
use crate::cli::handlers::test::requests::{get_user_id_from_daemon, update_run_name_for_test};

use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::info_message;
use crate::utils::system_info::check_sudo;
use crate::utils::workdir::TRACER_WORK_DIR;

use anyhow::Result;
use colored::Colorize;

/// TODO: fastquorum segfault on ARM mac; Rosetta/x86 pixi option may be needed.
pub async fn test(args: TracerCliTestArgs, config: Config, api_client: DaemonClient) -> Result<()> {
    // this is the entry function for the test command
    if !args.init_args.force_procfs && cfg!(target_os = "linux") {
        check_sudo("init");
    }

    let daemon_was_already_running = DaemonServer::is_running();

    let result = if daemon_was_already_running {
        run_test_with_existing_daemon(&api_client).await
    } else {
        run_test_with_new_daemon(args, config, &api_client).await
    };

    // Always show info
    info::info(&api_client, false).await;

    result
}

/// Initialize daemon with new pipeline name and run test pipeline
async fn run_test_with_new_daemon(
    args: TracerCliTestArgs,
    config: Config,
    api_client: &DaemonClient,
) -> Result<()> {
    info_message!("[run_test_with_new_daemon] Daemon is not running, starting new instance...");
    TRACER_WORK_DIR.init().expect("creating work files failed");

    // prepare test arguments
    let (mut init_args, selected_test_pipeline) = args.resolve_test_arguments()?;
    init_args.watch_dir = Some("/tmp/tracer".to_string());

    let new_test_pipeline_name = format!("test-{}", selected_test_pipeline.name());

    crate::cli::handlers::init::init_with(
        init_args,
        config,
        api_client,
        &new_test_pipeline_name,
    )
    .await?;

    // Run the pipeline after the daemon has been started
    let result = selected_test_pipeline.execute();

    info_message!("Shutting down daemon following test completion...");
    terminate::terminate(&api_client).await;

    result
}

/// Run test pipeline when daemon is already running
async fn run_test_with_existing_daemon(api_client: &DaemonClient) -> Result<()> {
    info_message!("Daemon is already running, executing fastquorum pipeline...");

    let fastquorum_pipeline = Pipeline::tracer(get_tracer_pipeline_path("fastquorum"))?;

    let user_id = get_user_id_from_daemon(api_client).await;
    update_run_name_for_test(api_client, &user_id).await;

    fastquorum_pipeline.execute()
}
