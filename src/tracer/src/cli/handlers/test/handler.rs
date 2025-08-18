use crate::cli::handlers::info;
use crate::cli::handlers::init::arguments::PromptMode;
use crate::cli::handlers::terminate;
use crate::cli::handlers::test::git::get_tracer_pipeline_path;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::cli::handlers::test::pixi;
use crate::cli::handlers::test::resolve_test_args::{resolve_test_arguments, TracerCliTestArgs};
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::info_message;
use crate::utils::command::check_status;
use crate::utils::system_info::check_sudo;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::warning_message;
use anyhow::Result;
use colored::Colorize;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

/// Single entry point to execute any pipeline variant.
fn execute_pipeline(pipeline: &Pipeline) -> Result<()> {
    match pipeline {
        Pipeline::LocalPixi { manifest, task, .. } => run_pixi_task(manifest.clone(), task.clone()),
        Pipeline::LocalNextflow { path, args } => run_nextflow(path, args),
        Pipeline::GithubNextflow { repo, args } => run_nextflow(repo, args),
        Pipeline::LocalTool { path, args } => run_tool(path, args),
    }
}

/// TODO: fastquorum segfault on ARM mac; Rosetta/x86 pixi option may be needed.
pub async fn test(args: TracerCliTestArgs, config: Config, api_client: DaemonClient) -> Result<()> {
    // this is the entry function for the test command
    if !args.init_args.force_procfs && cfg!(target_os = "linux") {
        check_sudo("init");
    }

    let result = if DaemonServer::is_running() {
        run_test_with_existing_daemon(&api_client).await
    } else {
        run_test_with_new_daemon(args, config, &api_client).await
    };

    // Always show info and cleanup if daemon is running
    info::info(&api_client, false).await;
    
    if DaemonServer::is_running() {
        info_message!("Shutting down daemon...");
        terminate::terminate(&api_client).await;
    }

    result
}

/// Initialize daemon and run test pipeline
async fn run_test_with_new_daemon(
    args: TracerCliTestArgs,
    config: Config,
    api_client: &DaemonClient
) -> Result<()> {
    info_message!("Daemon is not running, starting new instance...");
    TRACER_WORK_DIR.init().expect("creating work files failed");

    // prepare test arguments
    let mut init_args = resolve_test_arguments(args.clone());
    let selected_test_pipeline = Pipeline::select_test_pipeline(args.demo_pipeline_id, args.init_args.interactive_prompts)?;
    init_args.watch_dir = Some("/tmp/tracer".to_string());

    initialize_daemon(init_args, config, api_client, &selected_test_pipeline).await?;

    execute_pipeline_with_logging(&selected_test_pipeline)
}

/// Initialize daemon with proper configuration
async fn initialize_daemon(
    init_args: crate::cli::handlers::init::arguments::TracerCliInitArgs,
    config: Config,
    api_client: &DaemonClient,
    pipeline: &Pipeline,
) -> Result<()> {
    let default_prefix = format!("test-{}", pipeline.name());
    let confirm = init_args.interactive_prompts != PromptMode::None;

    crate::cli::handlers::init::init_with(
        init_args,
        config,
        api_client,
        &default_prefix,
        confirm,
    )
    .await
}

/// Execute pipeline with appropriate logging
fn execute_pipeline_with_logging(pipeline: &Pipeline) -> Result<()> {
    info_message!("Running pipeline...");
    let result = execute_pipeline(pipeline);

    if result.is_ok() {
        info_message!("Pipeline run completed successfully.");
    }

    result
}

/// Run test pipeline when daemon is already running
async fn run_test_with_existing_daemon(api_client: &DaemonClient) -> Result<()> {
    info_message!("Daemon is already running, executing fastquorum pipeline...");

    let fastquorum_pipeline = Pipeline::tracer(get_tracer_pipeline_path("fastquorum"))?;
    
    let user_id = get_user_id_from_daemon(api_client).await;
    update_run_name_for_test(api_client, &user_id).await;

    execute_pipeline_with_logging(&fastquorum_pipeline)
}

/// Get user ID from daemon with fallback to 'unknown'
async fn get_user_id_from_daemon(api_client: &DaemonClient) -> String {
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
async fn update_run_name_for_test(api_client: &DaemonClient, user_id: &str) {
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

/// Install pixi if necessary, then run task in manifest.
fn run_pixi_task(manifest: PathBuf, task: String) -> Result<()> {
    let pixi_path = which::which("pixi").unwrap_or_else(|_| {
        info_message!("Installing pixi...");
        // install() returns a PathBuf
        pixi::install().expect("pixi installation failed")
    });

    exec(
        Command::new(pixi_path)
            .arg("run")
            .arg("--manifest-path")
            .arg(manifest)
            .arg(task),
        "Pipeline run failed",
    )
}

/// Run a Nextflow pipeline (ensures nextflow exists first).
fn run_nextflow<S: AsRef<OsStr>>(pipeline: S, args: &Vec<String>) -> Result<()> {
    check_status(
        Command::new("nextflow").arg("-version").status(),
        "Nextflow not found",
    )?;

    exec(
        Command::new("nextflow").arg("run").args(args).arg(pipeline),
        "Pipeline run failed",
    )
}

/// Run an arbitrary tool with args.
fn run_tool<S: AsRef<OsStr>>(tool: S, args: &Vec<String>) -> Result<()> {
    exec(Command::new(tool).args(args), "Tool run failed")
}

/// Uniform spawn/wait + error mapping.
fn exec(cmd: &mut Command, fail_msg: &str) -> Result<()> {
    let status = cmd.spawn().and_then(|mut child| child.wait());
    check_status(status, fail_msg)
}