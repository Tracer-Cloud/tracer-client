use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::handlers::test::git::TracerPipelinesRepo;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::cli::handlers::INTERACTIVE_THEME;
use clap::Args;
use dialoguer::{Input, Select};
use std::path::PathBuf;

const DEFAULT_PIPELINE_NAME: &str = "fastquorum";

/// Executes a tool or pipeline with automatic tracing. Equivalent to
/// `tracer init && <run pipeline command> && tracer terminate`. Exactly one of the following
/// must be specified: --demo-pipeline-id, --nf-pipeline-path, --nf-pipeline-repo, or --tool-path.
#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliTestArgs {
    /// Name of example pipeline to test
    #[clap(short = 'd', long)]
    pub demo_pipeline_id: Option<String>,

    /// Path to a local pipeline to test
    #[clap(short = 'n', long)]
    pub nf_pipeline_path: Option<PathBuf>,

    /// Name of pixi task to run (if pipeline_path is specified and pixi.toml exists)
    #[clap(long)]
    pub pixi_task: Option<String>,

    /// Don't use pixi for local pipeline, even if pixi.toml exists
    #[clap(long)]
    pub no_pixi: bool,

    /// Name of a GitHub repo with a pipeline to test
    #[clap(short = 'r', long)]
    pub nf_pipeline_repo: Option<String>,

    /// Path to a local tool to test
    #[clap(short = 't', long)]
    pub tool_path: Option<PathBuf>,

    #[clap(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,

    #[command(flatten)]
    pub init_args: TracerCliInitArgs,
}

impl TracerCliTestArgs {
    pub fn finalize(self) -> (TracerCliInitArgs, Pipeline) {
        let theme = &*INTERACTIVE_THEME;

        let pipeline = if self.nf_pipeline_path.is_some() {
            if !self.no_pixi
                && self
                    .nf_pipeline_path
                    .as_ref()
                    .unwrap()
                    .join("pixi.toml")
                    .exists()
            {
                let task = self.prompt_for_pixi_task();
                Pipeline::local_pixi(self.nf_pipeline_path.unwrap(), &task).unwrap()
            } else {
                let args = if !self.args.is_empty() {
                    self.args
                } else {
                    self.prompt_for_args("pipeline")
                };
                Pipeline::LocalNextflow {
                    path: self.nf_pipeline_path.unwrap(),
                    args,
                }
            }
        } else if self.nf_pipeline_repo.is_some() {
            let args = if !self.args.is_empty() {
                self.args
            } else {
                self.prompt_for_args("pipeline")
            };
            Pipeline::GithubNextflow {
                repo: self.nf_pipeline_repo.unwrap(),
                args,
            }
        } else if self.tool_path.is_some() {
            let args = if !self.args.is_empty() {
                self.args
            } else {
                self.prompt_for_args("tool")
            };
            Pipeline::LocalTool {
                path: self.tool_path.unwrap(),
                args,
            }
        } else if self.init_args.non_interactive && self.demo_pipeline_id.is_none() {
            panic!("No pipeline specified")
        } else {
            println!("Syncing pipelines repo...");
            let pipelines_repo = TracerPipelinesRepo::new().expect("Failed to sync pipelines repo");

            let pipelines = pipelines_repo.list_pipelines();

            if !self.init_args.non_interactive {
                let mut pipeline_names: Vec<&str> = pipelines.iter().map(|p| p.name()).collect();
                pipeline_names.sort();
                let custom_index = pipeline_names.len();
                pipeline_names.push("Custom nextflow (local path, with pixi task)");
                pipeline_names.push("Custom nextflow (local path, host environment)");
                pipeline_names.push("Custom nextflow (GitHub repo)");
                pipeline_names.push("Custom tool (local path, host environment)");

                let pipeline_index = self
                    .demo_pipeline_id
                    .as_ref()
                    .map(|n| {
                        pipeline_names
                            .iter()
                            .position(|p| p == n)
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
                    let task = self.prompt_for_pixi_task();
                    Pipeline::local_pixi(pipeline_path, &task).expect("Invalid local pixi pipeline")
                } else if pipeline_index == custom_index + 1 {
                    let pipeline_path: String = Input::with_theme(&*INTERACTIVE_THEME)
                        .with_prompt("Enter custom pipeline local path")
                        .interact_text()
                        .expect("Error while prompting for pipeline path");
                    let args = if !self.args.is_empty() {
                        self.args
                    } else {
                        self.prompt_for_args("pipeline")
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
                        self.prompt_for_args("pipeline")
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
                        self.prompt_for_args("tool")
                    };
                    Pipeline::LocalTool {
                        path: tool_path.into(),
                        args,
                    }
                } else {
                    pipelines.into_iter().nth(pipeline_index).unwrap()
                }
            } else {
                let pipeline_name = self.demo_pipeline_id.as_ref().unwrap();
                if let Some(pipeline) = pipelines.into_iter().find(|p| p.name() == *pipeline_name) {
                    pipeline
                } else {
                    panic!("Invalid pipeline name {pipeline_name}")
                }
            }
        };

        pipeline.validate().expect("Invalid pipeline");

        let mut init_args = self.init_args;

        if init_args.pipeline_name.is_none() {
            init_args.pipeline_name = Some(pipeline.name().to_owned());
        }
        if init_args.run_name.is_none() {
            init_args.run_name = Some(format!("test-{}", pipeline.name()));
        }
        if init_args.tags.environment.is_none() {
            init_args.tags.environment = Some("local".into());
        }
        if init_args.tags.pipeline_type.is_none() {
            init_args.tags.pipeline_type = Some("preprocessing".into());
        }

        (init_args, pipeline)
    }

    fn prompt_for_pixi_task(&self) -> String {
        if self.init_args.non_interactive {
            return self
                .pixi_task
                .clone()
                .expect("No default pixi task specified");
        }
        let mut prompt = Input::with_theme(&*INTERACTIVE_THEME).with_prompt("Enter pixi task name");
        if let Some(default) = &self.pixi_task {
            prompt = prompt.default(default.clone());
        }
        prompt
            .interact_text()
            .expect("Error while prompting for pixi task")
    }

    fn prompt_for_args(&self, arg_type: &str) -> Vec<String> {
        if self.init_args.non_interactive {
            return vec![];
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::handlers::test::git::get_tracer_pipeline_path;

    #[test]
    fn test_finalize() {
        let args = TracerCliTestArgs {
            demo_pipeline_id: Some("fastquorum".to_string()),
            nf_pipeline_path: None,
            nf_pipeline_repo: None,
            tool_path: None,
            pixi_task: None,
            no_pixi: false,
            args: vec![],
            init_args: TracerCliInitArgs {
                non_interactive: false,
                log_level: "info".into(),
                ..Default::default()
            },
        };

        let (_, pipeline) = args.finalize();

        if let Pipeline::LocalPixi { path, .. } = &pipeline {
            assert_eq!(pipeline.name(), "fastquorum");
            assert_eq!(path, &get_tracer_pipeline_path("fastquorum"));
        } else {
            panic!("Expected local pipeline");
        }
    }
}
