use anyhow::{anyhow, bail, Context, Result};
use crate::cli::handlers::init::arguments::PromptMode;
use crate::cli::handlers::test::git::TracerPipelinesRepo;
use crate::cli::handlers::test::pixi;
use crate::cli::handlers::INTERACTIVE_THEME;
use crate::info_message;
use crate::utils::command::check_status;
use colored::Colorize;
use dialoguer::Select;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

const DEFAULT_PIPELINE: &str = "fastquorum";

pub enum Pipeline {
    LocalPixi {
        path: PathBuf,
        manifest: PathBuf,
        task: String,
    },
    LocalNextflow {
        path: PathBuf,
        args: Vec<String>,
    },
    GithubNextflow {
        repo: String,
        args: Vec<String>,
    },
    LocalTool {
        path: PathBuf,
        args: Vec<String>,
    },
}

impl Pipeline {
    pub fn tracer<P: Into<PathBuf>>(path: P) -> Result<Self> {
        Self::local_pixi(path, "pipeline")
    }

    pub fn local_pixi<P: Into<PathBuf>>(path: P, task: &str) -> Result<Self> {
        let path = path.into();
        let manifest = path.join("pixi.toml");
        Ok(Pipeline::LocalPixi {
            path,
            manifest,
            task: task.into(),
        })
    }

    pub fn name(&self) -> &str {
        match self {
            Pipeline::LocalPixi { path, .. } => path.file_name().unwrap().to_str().unwrap(),
            Pipeline::LocalNextflow { path, .. } => path.file_name().unwrap().to_str().unwrap(),
            Pipeline::LocalTool { path, .. } => path.file_name().unwrap().to_str().unwrap(),
            Pipeline::GithubNextflow { repo, .. } => repo,
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::LocalPixi {
                path,
                manifest,
                task: _,
            } => {
                if !path.exists() {
                    bail!("Pipeline path does not exist: {path:?}");
                }
                if !manifest.exists() {
                    bail!("Pixi manifest file does not exist: {manifest:?}");
                }
                // TODO: look for task in manifest
            }
            Self::LocalNextflow { path, .. } => {
                if !path.exists() {
                    bail!("Pipeline path does not exist: {path:?}");
                }
            }
            Self::GithubNextflow { repo: _, .. } => {
                // TODO: validate repo
            }
            Self::LocalTool { path, .. } => {
                if which::which(path.file_name().expect("Invalid file name")).is_err() {
                    bail!("Tool path does not exist: {path:?}");
                }
            }
        }
        Ok(())
    }

    /// Pure function for pipeline selection
    pub fn select_test_pipeline(demo_pipeline_id: Option<String>, interactive_prompts: PromptMode) -> Result<Pipeline> {
        let pipelines = sync_pipelines()?;
        let pipeline_name = select_pipeline_name(demo_pipeline_id, interactive_prompts, &pipelines)?;
        find_and_validate_pipeline(pipelines, &pipeline_name)
    }

    /// Single entry point to execute any pipeline variant.
    pub fn execute(&self) -> Result<()> {
        info_message!("Running pipeline...");

        let result = match self {
            Pipeline::LocalPixi { manifest, task, .. } => run_pixi_task(manifest.clone(), task.clone()),
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

// Pipeline selection helper functions
fn sync_pipelines() -> Result<Vec<Pipeline>> {
    Ok(TracerPipelinesRepo::new()?.list_pipelines())
}

fn select_pipeline_name(demo_pipeline_id: Option<String>, interactive_prompts: PromptMode, pipelines: &[Pipeline]) -> Result<String> {
    match demo_pipeline_id {
        Some(name) => validate_pipeline_exists(pipelines, &name),
        None => choose_pipeline_with_interactive_prompt(interactive_prompts, pipelines),
    }
}

fn validate_pipeline_exists(pipelines: &[Pipeline], name: &str) -> Result<String> {
    pipelines.iter()
        .find(|p| p.name() == name)
        .map(|_| name.to_string())
        .ok_or_else(|| anyhow!("pipeline '{}' not found", name))
}

fn choose_pipeline_with_interactive_prompt(interactive_prompts: PromptMode, pipelines: &[Pipeline]) -> Result<String> {
    let is_interactive = interactive_prompts != PromptMode::None;

    if is_interactive && pipelines.len() > 1 {
        prompt_for_pipeline_selection(pipelines)
    } else {
        get_default_or_first_pipeline(pipelines)
    }
}

fn prompt_for_pipeline_selection(pipelines: &[Pipeline]) -> Result<String> {
    let mut names: Vec<&str> = pipelines.iter().map(|p| p.name()).collect();
    names.sort_unstable();

    let default_idx = names.iter()
        .position(|&name| name == DEFAULT_PIPELINE)
        .unwrap_or(0);

    let selection = Select::with_theme(&*INTERACTIVE_THEME)
        .with_prompt("Select pipeline to run")
        .items(&names)
        .default(default_idx)
        .interact()
        .context("pipeline selection failed")?;

    Ok(names[selection].to_string())
}

fn get_default_or_first_pipeline(pipelines: &[Pipeline]) -> Result<String> {
    pipelines.iter()
        .find(|p| p.name() == DEFAULT_PIPELINE)
        .or_else(|| pipelines.first())
        .map(|p| p.name().to_string())
        .ok_or_else(|| anyhow!("no pipelines available"))
}

fn find_and_validate_pipeline(pipelines: Vec<Pipeline>, name: &str) -> Result<Pipeline> {
    pipelines.into_iter()
        .find(|p| p.name() == name)
        .ok_or_else(|| anyhow!("pipeline '{}' not found", name))
        .and_then(|p| p.validate().map(|_| p))
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
