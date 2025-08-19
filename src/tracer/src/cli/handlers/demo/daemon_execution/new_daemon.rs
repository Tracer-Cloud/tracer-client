use crate::cli::handlers::demo::pipeline::Pipeline;
use crate::cli::handlers::info;
use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::handlers::terminate;

use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::info_message;
use crate::utils::workdir::TRACER_WORK_DIR;

use anyhow::Result;
use colored::Colorize;

pub async fn run_demo_with_new_daemon(
    init_args: TracerCliInitArgs,
    config: Config,
    api_client: &DaemonClient,
    selected_demo_pipeline: Pipeline,
) -> Result<()> {
    let configured_args = prepare_demo_environment(init_args, &selected_demo_pipeline)?;

    // Init daemon, run pipeline, cleanup
    initialize_daemon_for_demo(configured_args, config, api_client).await?;
    execute_pipeline_and_report(selected_demo_pipeline, api_client).await?;
    cleanup_daemon(api_client).await;

    Ok(())
}

fn prepare_demo_environment(
    mut init_args: TracerCliInitArgs,
    pipeline: &Pipeline,
) -> Result<TracerCliInitArgs> {
    TRACER_WORK_DIR
        .init()
        .map_err(|_| anyhow::anyhow!("Failed to create tracer work directory"))?;

    init_args.configure_for_test();

    // For demo scenarios, we want the pipeline name to be "{environment}-demo-{pipeline_id}-{user_id}"
    // We'll set a special marker that the resolver will recognize and expand properly
    if init_args.pipeline_name.is_none() {
        // Set a special marker that the resolver will recognize for demo pipelines
        // Format: "demo-pipeline:{pipeline_id}" -> resolver expands to "{environment}-demo-{pipeline_id}-{user_id}"
        let pipeline_name = format!("demo-pipeline:{}", pipeline.name());
        init_args.pipeline_name = Some(pipeline_name);
    }

    Ok(init_args)
}

async fn initialize_daemon_for_demo(
    init_args: TracerCliInitArgs,
    config: Config,
    api_client: &DaemonClient,
) -> Result<()> {
    info_message!("Starting daemon for demo execution...");
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
