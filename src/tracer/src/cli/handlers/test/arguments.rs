use crate::cli::handlers::test::git::TracerPipelinesRepo;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::cli::handlers::INTERACTIVE_THEME;
use clap::Args;
use dialoguer::{Input, Select};
use std::path::PathBuf;

const DEFAULT_PIPELINE_NAME: &str = "fastquorum";

#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliTestArgs {
    /// Name of example workflow to test
    #[clap(long, short = 'n')]
    pub pipeline_name: Option<String>,

    /// Path to a local pipeline to test
    #[clap(long, short = 'p')]
    pub pipeline_path: Option<PathBuf>,

    /// Don't use pixi for local pipeline, even if pixi.toml exists
    #[clap(long)]
    pub no_pixi: bool,

    /// Name of a GitHub repo with a pipeline to test
    #[clap(long, short = 'r')]
    pub pipeline_repo: Option<String>,

    /// Path to a local tool to test
    #[clap(long, short)]
    pub tool_path: Option<PathBuf>,

    #[clap(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,

    /// Do not prompt for missing inputs
    #[clap(short = 'f', long)]
    pub non_interactive: bool,

    /// Capture logs at the specified level and above (default: info)
    /// Valid values: trace, debug, info, warn, error
    /// Output will be written to `daemon.log` in the working directory.
    #[clap(long, default_value = "info")]
    pub log_level: String,
}

impl TracerCliTestArgs {
    pub fn finalize(self) -> FinalizedTestArgs {
        let theme = &*INTERACTIVE_THEME;

        println!("Syncing pipelines repo...");
        let pipelines_repo = TracerPipelinesRepo::new().expect("Failed to sync pipelines repo");

        let pipelines = pipelines_repo.list_pipelines();

        let mut pipeline_names: Vec<&str> = pipelines.iter().map(|p| p.name()).collect();
        pipeline_names.sort();
        let custom_index = pipeline_names.len();
        pipeline_names.push("Custom nextflow (local path, with pixi task)");
        pipeline_names.push("Custom nextflow (local path, host environment)");
        pipeline_names.push("Custom nextflow (GitHub repo)");
        pipeline_names.push("Custom tool (local path, host environment)");

        let pipeline = if self.pipeline_path.is_some() {
            if !self.no_pixi
                && self
                    .pipeline_path
                    .as_ref()
                    .unwrap()
                    .join("pixi.toml")
                    .exists()
            {
                let task = Self::prompt_for_pixi_task();
                Pipeline::local_pixi(self.pipeline_path.unwrap(), &task).unwrap()
            } else {
                let args = if !self.args.is_empty() {
                    self.args
                } else {
                    Self::prompt_for_args("pipeline")
                };
                Pipeline::LocalNextflow {
                    path: self.pipeline_path.unwrap(),
                    args,
                }
            }
        } else if self.pipeline_repo.is_some() {
            let args = if !self.args.is_empty() {
                self.args
            } else {
                Self::prompt_for_args("pipeline")
            };
            Pipeline::GithubNextflow {
                repo: self.pipeline_repo.unwrap(),
                args,
            }
        } else if self.tool_path.is_some() {
            let args = if !self.args.is_empty() {
                self.args
            } else {
                Self::prompt_for_args("tool")
            };
            Pipeline::LocalTool {
                path: self.tool_path.unwrap(),
                args,
            }
        } else {
            let pipeline_index = self
                .pipeline_name
                .map(|n| {
                    pipeline_names
                        .iter()
                        .position(|p| p == &n)
                        .expect("Invalid pipline name")
                })
                .unwrap_or_else(|| {
                    let default_index = pipeline_names
                        .iter()
                        .position(|n| *n == DEFAULT_PIPELINE_NAME)
                        .unwrap_or(0);
                    Select::with_theme(theme)
                        .with_prompt("Select pipeline to run")
                        .items(&pipeline_names)
                        .default(default_index)
                        .interact()
                        .expect("Error while prompting for pipeline name")
                });

            if pipeline_index == custom_index {
                let pipeline_path: String = Input::with_theme(&*INTERACTIVE_THEME)
                    .with_prompt("Enter custom pipeline local path")
                    .interact_text()
                    .expect("Error while prompting for pipeline path");
                let task = Self::prompt_for_pixi_task();
                Pipeline::local_pixi(pipeline_path, &task).expect("Invalid local pixi pipeline")
            } else if pipeline_index == custom_index + 1 {
                let pipeline_path: String = Input::with_theme(&*INTERACTIVE_THEME)
                    .with_prompt("Enter custom pipeline local path")
                    .interact_text()
                    .expect("Error while prompting for pipeline path");
                let args = if !self.args.is_empty() {
                    self.args
                } else {
                    Self::prompt_for_args("pipeline")
                };
                Pipeline::LocalNextflow {
                    path: pipeline_path.into(),
                    args,
                }
            } else if pipeline_index == custom_index + 2 {
                let repo = Input::with_theme(&*INTERACTIVE_THEME)
                    .with_prompt("Enter custom pipeline GitHub repo")
                    .interact_text()
                    .expect("Error while prompting for pipeline repo");
                let args = if !self.args.is_empty() {
                    self.args
                } else {
                    Self::prompt_for_args("pipeline")
                };
                Pipeline::GithubNextflow { repo, args }
            } else if pipeline_index == custom_index + 3 {
                let tool_path: String = Input::with_theme(&*INTERACTIVE_THEME)
                    .with_prompt("Enter custom tool local path")
                    .interact_text()
                    .expect("Error while prompting for tool path");
                let args = if !self.args.is_empty() {
                    self.args
                } else {
                    Self::prompt_for_args("tool")
                };
                Pipeline::LocalTool {
                    path: tool_path.into(),
                    args,
                }
            } else {
                pipelines.into_iter().nth(pipeline_index).unwrap()
            }
        };

        pipeline.validate().expect("Invalid pipeline");

        FinalizedTestArgs {
            pipeline,
            log_level: self.log_level,
        }
    }

    fn prompt_for_pixi_task() -> String {
        Input::with_theme(&*INTERACTIVE_THEME)
            .with_prompt("Enter pixi task name")
            .interact_text()
            .expect("Error while prompting for pixi task")
    }

    fn prompt_for_args(arg_type: &str) -> Vec<String> {
        let args_str: String = Input::with_theme(&*INTERACTIVE_THEME)
            .with_prompt(format!(
                "Enter custom {} arguments (separated by spaces)",
                arg_type
            ))
            .allow_empty(true)
            .interact_text()
            .expect("Error while prompting for arguments");
        shlex::split(&args_str).expect("Error parsing arguments")
    }
}

pub struct FinalizedTestArgs {
    pub pipeline: Pipeline,
    pub log_level: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::handlers::test::git::get_tracer_pipeline_path;

    #[test]
    fn test_finalize() {
        let args = TracerCliTestArgs {
            pipeline_name: Some("fastquorum".to_string()),
            pipeline_path: None,
            pipeline_repo: None,
            tool_path: None,
            no_pixi: false,
            args: vec![],
            non_interactive: true,
            log_level: "info".into(),
        };

        let finalized_args = args.finalize();

        if let Pipeline::LocalPixi { path, .. } = &finalized_args.pipeline {
            assert_eq!(finalized_args.pipeline.name(), "fastquorum");
            assert_eq!(path, &get_tracer_pipeline_path("fastquorum"));
        } else {
            panic!("Expected local pipeline");
        }
    }
}
