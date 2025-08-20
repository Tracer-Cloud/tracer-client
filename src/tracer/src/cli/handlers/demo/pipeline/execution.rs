use super::pixi;
use super::Pipeline;
use crate::info_message;
use crate::utils::command::check_status;
use crate::utils::Sentry;
use anyhow::Result;
use colored::Colorize;
use serde_json::json;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

impl Pipeline {
    /// Single entry point to execute any pipeline variant.
    pub fn execute(&self) -> Result<()> {
        info_message!("Running pipeline...");

        // Add pipeline context to Sentry
        Sentry::add_context(
            "Pipeline Execution",
            json!({
                "pipeline_name": self.name(),
                "pipeline_type": match self {
                    Pipeline::LocalPixi { .. } => "LocalPixi",
                    Pipeline::LocalNextflow { .. } => "LocalNextflow",
                    Pipeline::GithubNextflow { .. } => "GithubNextflow",
                    Pipeline::LocalTool { .. } => "LocalTool",
                }
            }),
        );

        let result = match self {
            Pipeline::LocalPixi { manifest, task, .. } => {
                run_pixi_task(manifest.clone(), task.clone())
            }
            Pipeline::LocalNextflow { path, args } => run_nextflow(path, args),
            Pipeline::GithubNextflow { repo, args } => run_nextflow(repo, args),
            Pipeline::LocalTool { path, args } => run_tool(path, args),
        };

        match &result {
            Ok(_) => {
                info_message!("Pipeline run completed successfully.");
                Sentry::capture_message(
                    &format!("Pipeline '{}' executed successfully", self.name()),
                    sentry::Level::Info,
                );
            }
            Err(e) => {
                // Capture pipeline execution error to Sentry
                Sentry::add_extra(
                    "error_details",
                    json!({
                        "error_message": e.to_string(),
                        "pipeline_name": self.name(),
                        "error_chain": format!("{:?}", e)
                    }),
                );

                Sentry::capture_message(
                    &format!("Pipeline '{}' execution failed: {}", self.name(), e),
                    sentry::Level::Error,
                );
            }
        }

        result
    }
}

// Pipeline execution helper functions

/// Install pixi if necessary, then run task in manifest.
fn run_pixi_task(manifest: PathBuf, task: String) -> Result<()> {
    let pixi_path = match which::which("pixi") {
        Ok(path) => {
            info_message!("Using system pixi: {}", path.display());
            path
        }
        Err(_) => {
            info_message!("Pixi not found in PATH, installing to local directory...");
            let installed_path = pixi::install_pixi()
                .map_err(|e| anyhow::anyhow!("Failed to install pixi: {}", e))?;
            info_message!("Pixi installed to: {}", installed_path.display());
            installed_path
        }
    };

    info_message!(
        "Running pixi task '{}' with manifest: {}",
        task,
        manifest.display()
    );

    // Prepare the command with proper environment
    let mut cmd = Command::new(&pixi_path);
    cmd.arg("run")
        .arg("--manifest-path")
        .arg(manifest)
        .arg(task);

    // If we installed pixi locally, add its directory to PATH so that
    // any scripts executed by pixi can also find pixi
    if let Some(pixi_dir) = pixi_path.parent() {
        if let Ok(current_path) = std::env::var("PATH") {
            let new_path = format!("{}:{}", pixi_dir.display(), current_path);
            cmd.env("PATH", new_path);
            info_message!("Added pixi directory to PATH: {}", pixi_dir.display());
        }
    }

    exec(&mut cmd, "Pipeline run failed")
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
