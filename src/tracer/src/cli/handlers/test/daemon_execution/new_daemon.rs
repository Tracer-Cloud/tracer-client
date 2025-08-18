use crate::cli::handlers::info;
use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::handlers::terminate;
use crate::cli::handlers::test::pipeline::Pipeline;

use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::info_message;
use crate::utils::workdir::TRACER_WORK_DIR;

use anyhow::Result;
use colored::Colorize;

pub async fn run_test_with_new_daemon(
    init_args: TracerCliInitArgs,
    config: Config,
    api_client: &DaemonClient,
    selected_test_pipeline: Pipeline,
) -> Result<()> {
    let configured_args = prepare_test_environment(init_args)?;

    // Init daemon, run pipeline, cleanup
    initialize_daemon_for_testing(configured_args, config, api_client).await?;
    execute_pipeline_and_report(selected_test_pipeline, api_client).await?;
    cleanup_daemon(api_client).await;

    Ok(())
}

fn prepare_test_environment(mut init_args: TracerCliInitArgs) -> Result<TracerCliInitArgs> {
    TRACER_WORK_DIR
        .init()
        .map_err(|_| anyhow::anyhow!("Failed to create tracer work directory"))?;

    init_args.configure_for_test();

    // For test scenarios, we want the pipeline name to be "test-{user_id}"
    // Since we removed the duplicate user_id resolution, we'll let the init flow handle this
    // by setting a placeholder that indicates this is a test
    if init_args.pipeline_name.is_none() {
        // This will be updated by the resolver to include the actual user_id
        init_args.pipeline_name = Some("test".to_string());
    }

    Ok(init_args)
}

async fn initialize_daemon_for_testing(
    init_args: TracerCliInitArgs,
    config: Config,
    api_client: &DaemonClient,
) -> Result<()> {
    info_message!("Starting daemon for test execution...");
    crate::cli::handlers::init::init(init_args, config, api_client).await
}

async fn execute_pipeline_and_report(pipeline: Pipeline, api_client: &DaemonClient) -> Result<()> {
    pipeline.execute()?;
    info::info(api_client, false).await;
    Ok(())
}

async fn cleanup_daemon(api_client: &DaemonClient) {
    info_message!("Cleaning up daemon...");
    terminate::terminate(api_client).await;
}
