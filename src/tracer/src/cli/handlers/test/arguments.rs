use crate::cli::handlers::init::arguments::{PromptMode, TracerCliInitArgs};
use crate::cli::handlers::test::git::TracerPipelinesRepo;
use crate::cli::handlers::test::pipeline::Pipeline;
use crate::cli::handlers::INTERACTIVE_THEME;
use clap::Args;
use dialoguer::Select;

const DEFAULT_PIPELINE_NAME: &str = "fastquorum";

/// Executes a tool or pipeline with automatic tracing. Equivalent to
/// `tracer init && <run pipeline command> && tracer terminate`. Exactly one of the following
/// must be specified: --demo-pipeline-id, --nf-pipeline-path, --nf-pipeline-repo, or --tool-path.
#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliTestArgs {
    /// Name of example pipeline to test
    #[clap(short = 'd', long)]
    pub demo_pipeline_id: Option<String>,

    #[command(flatten)]
    pub init_args: TracerCliInitArgs,
}

impl TracerCliTestArgs {
    pub fn finalize(self) -> (TracerCliInitArgs, Pipeline) {
        let theme = &*INTERACTIVE_THEME;
        let non_interactive = self.init_args.interactive_prompts == PromptMode::None;

        println!("Syncing pipelines repo...");
        let pipelines_repo = TracerPipelinesRepo::new().expect("Failed to sync pipelines repo");

        let pipelines = pipelines_repo.list_pipelines();
        let mut pipeline_names: Vec<&str> = pipelines.iter().map(|p| p.name()).collect();

        let pipeline = if !non_interactive && pipelines.len() > 1 {
            pipeline_names.sort();

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

            pipelines.into_iter().nth(pipeline_index).unwrap()
        } else {
            let pipeline_name = self
                .demo_pipeline_id
                .unwrap_or(DEFAULT_PIPELINE_NAME.to_string());
            if let Some(pipeline) = pipelines.into_iter().find(|p| p.name() == pipeline_name) {
                pipeline
            } else {
                panic!("Invalid pipeline name {pipeline_name}")
            }
        };

        pipeline.validate().expect("Invalid pipeline");

        (self.init_args, pipeline)
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
            init_args: TracerCliInitArgs {
                interactive_prompts: PromptMode::Minimal,
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
