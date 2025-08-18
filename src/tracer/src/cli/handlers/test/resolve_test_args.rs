use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::handlers::test::pipeline::Pipeline;
use anyhow::Result;
use clap::Args;

/// Test a pipeline with automatic tracing
#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliTestArgs {
    /// Pipeline to test (defaults to interactive selection)
    #[clap(short = 'd', long)]
    pub demo_pipeline_id: Option<String>,

    #[command(flatten)]
    pub init_args: TracerCliInitArgs,
}

// Pure function for argument resolution
pub fn resolve_test_arguments(args: TracerCliTestArgs) -> TracerCliInitArgs {
    args.init_args
}

// Legacy function for backward compatibility
pub fn resolve_pipeline_for_testing(args: TracerCliTestArgs) -> Result<(TracerCliInitArgs, Pipeline)> {
    let interactive_prompts = args.init_args.interactive_prompts.clone();
    let demo_pipeline_id = args.demo_pipeline_id.clone();
    let init_args = resolve_test_arguments(args);
    let pipeline = Pipeline::select_test_pipeline(demo_pipeline_id, interactive_prompts)?;

    Ok((init_args, pipeline))
}

// Implementation trait for backward compatibility
impl TracerCliTestArgs {
    pub fn resolve_init_arguments_and_select_test_pipeline(self) -> Result<(TracerCliInitArgs, Pipeline)> {
        resolve_pipeline_for_testing(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::handlers::init::arguments::PromptMode;
    use crate::cli::handlers::test::git::get_tracer_pipeline_path;

    #[test]
    fn test_resolve_fastquorum() {
        let args = TracerCliTestArgs {
            demo_pipeline_id: Some("fastquorum".to_string()),
            init_args: TracerCliInitArgs {
                interactive_prompts: PromptMode::Minimal,
                log_level: "info".into(),
                ..Default::default()
            },
        };

        let (_, pipeline) = resolve_pipeline_for_testing(args)
            .expect("failed to resolve pipeline");

        assert_eq!(pipeline.name(), "fastquorum");
        
        if let Pipeline::LocalPixi { path, .. } = &pipeline {
            assert_eq!(path, &get_tracer_pipeline_path("fastquorum"));
        } else {
            panic!("expected LocalPixi pipeline");
        }
    }
}