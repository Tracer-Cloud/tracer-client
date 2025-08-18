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

impl TracerCliTestArgs {
    /// Resolve init arguments and select test pipeline
    pub fn resolve_test_arguments(self) -> Result<(TracerCliInitArgs, Pipeline)> {
        let interactive_prompts = self.init_args.interactive_prompts.clone();
        let pipeline = Pipeline::select_test_pipeline(self.demo_pipeline_id, interactive_prompts)?;
        Ok((self.init_args, pipeline))
    }
}
