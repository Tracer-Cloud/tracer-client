use crate::cli::handlers::demo::pipeline::Pipeline;
use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use anyhow::Result;
use clap::Args;

/// Run demo pipelines with automatic tracing
#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliDemoArgs {
    /// Pipeline to demo (defaults to interactive selection)
    #[clap(short = 'd', long)]
    pub demo_pipeline_id: Option<String>,

    #[command(flatten)]
    pub init_args: TracerCliInitArgs,
}

impl TracerCliDemoArgs {
    /// Resolve init arguments and select demo pipeline
    pub fn resolve_demo_arguments(self) -> Result<(TracerCliInitArgs, Pipeline)> {
        let interactive_prompts = self.init_args.interactive_prompts.clone();
        let pipeline = Pipeline::select_demo_pipeline(self.demo_pipeline_id, interactive_prompts)?;
        Ok((self.init_args, pipeline))
    }
}
