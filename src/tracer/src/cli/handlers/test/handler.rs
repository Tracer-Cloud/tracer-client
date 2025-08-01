use crate::cli::handlers::info;
use crate::cli::handlers::init::arguments::PromptMode;
use crate::cli::handlers::terminate;
use crate::cli::handlers::test::arguments::TracerCliTestArgs;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::cli::handlers::test::pixi;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::utils::command::check_status;
use crate::utils::system_info::check_sudo;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::warning_message;
use anyhow::Result;
use colored::Colorize;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

/// TODO: I am getting a segfault running fastquorum on ARM mac
/// It works if I run with Rosetta emulation
/// env /usr/bin/arch -x86_64 -c -e TERM=$TERM /bin/sh --login
/// We may want to offer The option to install the x86 version of
/// pixi and run nextflow under x86 emulation
pub async fn test(
    args: TracerCliTestArgs,
    config: Config,
    api_client: DaemonClient,
) -> anyhow::Result<()> {
    if !args.init_args.force_procfs && cfg!(target_os = "linux") {
        // Check if running with sudo
        check_sudo("init");
    }

    // Create necessary files for logging and daemonizing
    TRACER_WORK_DIR
        .init()
        .expect("Error while creating necessary files");

    // Check for port conflict before starting daemon
    if DaemonServer::is_running() {
        warning_message!("Daemon server is already running, trying to terminate it...");
        if !terminate::terminate(&api_client).await {
            return Ok(());
        }
    }

    // Finalize test args
    let (init_args, pipeline) = args.finalize();
    let non_interactive = init_args.non_interactive;
    let prompt_mode = if non_interactive {
        PromptMode::Never
    } else {
        PromptMode::Always
    };

    // init tracer run
    crate::cli::handlers::init::init_with_default_prompt(
        init_args,
        config,
        &api_client,
        prompt_mode,
    )
    .await?;

    // run the pipeline
    println!("Running pipeline...");
    let result = match pipeline {
        Pipeline::LocalPixi { manifest, task, .. } => run_pixi_task(manifest, task),
        Pipeline::LocalNextflow { path, args } => run_nextflow(path, args),
        Pipeline::GithubNextflow { repo, args } => run_nextflow(repo, args),
        Pipeline::LocalTool { path, args } => run_tool(path, args),
    };

    if result.is_ok() {
        println!("Pipeline run completed successfully.");
    }

    info::info(&api_client, false).await;

    if DaemonServer::is_running() {
        println!("Shutting down daemon...");
        terminate::terminate(&api_client).await;
    }

    result
}

/// Install pixi if necessary, then run the specified task in the specified manifest
fn run_pixi_task(manifest: PathBuf, task: String) -> Result<()> {
    // install pixi if it doesn't exist in the path
    let pixi_path = if let Ok(path) = which::which("pixi") {
        path
    } else {
        println!("Installing pixi...");
        pixi::install()?
    };

    let status = Command::new(pixi_path)
        .arg("run")
        .arg("--manifest-path")
        .arg(manifest)
        .arg(task)
        .spawn()
        .and_then(|mut child| child.wait());
    check_status(status, "Pipeline run failed")
}

/// Run the pipeline with nextflow in the host environment with the specified arguments
fn run_nextflow<S: AsRef<OsStr>>(pipeline: S, args: Vec<String>) -> Result<()> {
    check_status(
        Command::new("nextflow").arg("-version").status(),
        "Nextflow not found",
    )?;

    let status = Command::new("nextflow")
        .arg("run")
        .args(args)
        .arg(pipeline)
        .spawn()
        .and_then(|mut child| child.wait());
    check_status(status, "Pipeline run failed")
}

/// Run the pipeline with nextflow in the host environment with the specified arguments
fn run_tool<S: AsRef<OsStr>>(tool: S, args: Vec<String>) -> Result<()> {
    let status = Command::new(tool)
        .args(args)
        .spawn()
        .and_then(|mut child| child.wait());
    check_status(status, "Tool run failed")
}
