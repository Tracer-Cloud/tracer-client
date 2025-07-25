use crate::cli::handlers::test::git::TracerPipelinesRepo;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::cli::handlers::INTERACTIVE_THEME;
use clap::Args;
use dialoguer::{Input, Select};

const DEFAULT_PIPELINE_NAME: &str = "fastquorum";

#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliTestArgs {
    /// Name of example workflow to test
    #[clap(long, short)]
    pub pipeline_name: Option<String>,
    /// Do not prompt for missing inputs
    #[clap(short = 'f', long)]
    pub non_interactive: bool,
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

        let pipeline = if pipeline_index == custom_index {
            let pipeline_path: String = Input::with_theme(&*INTERACTIVE_THEME)
                .with_prompt("Enter custom pipeline local path")
                .interact_text()
                .expect("Error while prompting for pipeline path");
            let task: String = Input::with_theme(&*INTERACTIVE_THEME)
                .with_prompt("Pixi task name")
                .interact_text()
                .expect("Error while prompting for pixi task");
            Pipeline::local_pixi(pipeline_path, &task).expect("Invalid local pixi pipeline")
        } else if pipeline_index == custom_index + 1 {
            let pipeline_path: String = Input::with_theme(&*INTERACTIVE_THEME)
                .with_prompt("Enter custom pipeline local path")
                .interact_text()
                .expect("Error while prompting for pipeline path");
            let args_str: String = Input::with_theme(&*INTERACTIVE_THEME)
                .with_prompt("Enter custom pipeline arguments (separated by spaces)")
                .allow_empty(true)
                .interact_text()
                .expect("Error while prompting for pipeline arguments");
            let args = shlex::split(&args_str).expect("Error parsing arguments");
            Pipeline::LocalCustom {
                path: pipeline_path.into(),
                args,
            }
        } else if pipeline_index == custom_index + 2 {
            let repo = Input::with_theme(&*INTERACTIVE_THEME)
                .with_prompt("Enter custom pipeline GitHub repo")
                .interact_text()
                .expect("Error while prompting for pipeline repo");
            let args_str: String = Input::with_theme(&*INTERACTIVE_THEME)
                .with_prompt("Enter custom pipeline arguments (separated by spaces)")
                .allow_empty(true)
                .interact_text()
                .expect("Error while prompting for pipeline arguments");
            let args = shlex::split(&args_str).expect("Error parsing arguments");
            Pipeline::GitHub { repo, args }
        } else if pipeline_index == custom_index + 3 {
            let tool_path: String = Input::with_theme(&*INTERACTIVE_THEME)
                .with_prompt("Enter custom tool local path")
                .interact_text()
                .expect("Error while prompting for tool path");
            let args_str: String = Input::with_theme(&*INTERACTIVE_THEME)
                .with_prompt("Enter custom tool arguments (separated by spaces)")
                .allow_empty(true)
                .interact_text()
                .expect("Error while prompting for tool arguments");
            let args = shlex::split(&args_str).expect("Error parsing arguments");
            Pipeline::LocalTool {
                path: tool_path.into(),
                args,
            }
        } else {
            pipelines.into_iter().nth(pipeline_index).unwrap()
        };

        FinalizedTestArgs { pipeline }
    }
}

pub struct FinalizedTestArgs {
    pub pipeline: Pipeline,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::handlers::test::git::get_tracer_pipelines_repo_path;

    #[test]
    fn test_finalize() {
        let args = TracerCliTestArgs {
            pipeline_name: Some("fastquorum".to_string()),
            non_interactive: true,
        };

        let finalized_args = args.finalize();

        if let Pipeline::LocalPixi { path, .. } = &finalized_args.pipeline {
            assert_eq!(finalized_args.pipeline.name(), "fastquorum");
            assert_eq!(
                path,
                &get_tracer_pipelines_repo_path().join("shared/fastquorum")
            );
        } else {
            panic!("Expected local pipeline");
        }
    }
}
