use crate::cli::handlers::info;
use crate::cli::handlers::init::arguments::{PromptMode, TracerCliInitArgs};
use crate::cli::handlers::test::arguments::TracerCliTestArgs;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::process_identification::types::pipeline_tags::PipelineTags;
use crate::utils::system_info::check_sudo;
use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::Result;
use std::ffi::OsStr;
use std::io;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

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
    // Check if running with sudo
    check_sudo("init");

    // Create necessary files for logging and daemonizing
    TRACER_WORK_DIR
        .init()
        .expect("Error while creating necessary files");

    let non_interactive = args.non_interactive;

    // Check for port conflict before starting daemon
    DaemonServer::shutdown_if_running().await?;

    // Finalize test args
    let test_args = args.finalize();

    // Prompt user for init args
    let tags = PipelineTags {
        environment: Some("local".into()),
        pipeline_type: Some("preprocessing".into()), // TODO: map pipeline name to pipeline type
        ..Default::default()
    };
    let prompt_mode = if non_interactive {
        PromptMode::Never
    } else {
        PromptMode::Always
    };
    let init_args = TracerCliInitArgs {
        pipeline_name: Some(test_args.pipeline.name().to_owned()),
        run_name: Some(format!("test-{}", test_args.pipeline.name())),
        tags,
        non_interactive,
        ..Default::default()
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
    let result = match test_args.pipeline {
        Pipeline::LocalPixi { manifest, task, .. } => run_pixi_task(manifest, task),
        Pipeline::LocalCustom { path, args } => run_nextflow(path, args),
        Pipeline::GitHub { repo, args } => run_nextflow(repo, args),
        Pipeline::LocalTool { path, args } => run_tool(path, args),
    };

    if result.is_ok() {
        println!("Pipeline run completed successfully.");
        if let Err(e) = info::info(&api_client, false).await {
            println!("Failed to show tracer info: {e}");
        }
    }

    println!("Shutting down daemon...");
    if let Err(e) = DaemonServer::shutdown_if_running().await {
        println!("Failed to shutdown daemon: {e}");
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
        let install_cmd = "curl -fsSL https://pixi.sh/install.sh | bash";
        let pixi_dir = TRACER_WORK_DIR.path.join(".pixi");
        let status = Command::new("sh")
            .arg("-c")
            .arg(install_cmd)
            .env("PIXI_HOME", &pixi_dir)
            .status();
        check_status(status, "Failed to install pixi")?;
        pixi_dir.join("bin/pixi")
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

fn check_status(status: io::Result<ExitStatus>, err_msg: &str) -> Result<()> {
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(anyhow::anyhow!("{err_msg}: {status}")),
        Err(e) => Err(anyhow::anyhow!("{err_msg}: {e}")),
    }
}
