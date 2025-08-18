use super::pixi;
use super::Pipeline;
use crate::info_message;
use crate::utils::command::check_status;
use anyhow::Result;
use colored::Colorize;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

impl Pipeline {
    /// Single entry point to execute any pipeline variant.
    pub fn execute(&self) -> Result<()> {
        info_message!("Running pipeline...");

        let result = match self {
            Pipeline::LocalPixi { manifest, task, .. } => {
                run_pixi_task(manifest.clone(), task.clone())
            }
            Pipeline::LocalNextflow { path, args } => run_nextflow(path, args),
            Pipeline::GithubNextflow { repo, args } => run_nextflow(repo, args),
            Pipeline::LocalTool { path, args } => run_tool(path, args),
        };

        if result.is_ok() {
            info_message!("Pipeline run completed successfully.");
        }

        result
    }
}

// Pipeline execution helper functions

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
