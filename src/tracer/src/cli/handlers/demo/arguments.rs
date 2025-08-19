use crate::cli::handlers::demo::pipeline::Pipeline;
use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use anyhow::Result;
use clap::{Args, Subcommand};

/// Run demo pipelines with automatic tracing
#[derive(Args, Debug, Clone)]
pub struct TracerCliDemoArgs {
    #[command(subcommand)]
    pub command: DemoCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DemoCommand {
    /// Run the fastquorum demo pipeline
    Fastquorum {
        #[command(flatten)]
        init_args: TracerCliInitArgs,
    },
    /// Run the WDL demo pipeline
    Wdl {
        #[command(flatten)]
        init_args: TracerCliInitArgs,
    },
    /// List available demo pipelines
    List,
    /// Run any available demo pipeline by name
    Run {
        /// Name of the pipeline to run
        name: String,
        #[command(flatten)]
        init_args: TracerCliInitArgs,
    },
}

impl TracerCliDemoArgs {
    /// Resolve init arguments and select demo pipeline
    pub fn resolve_demo_arguments(self) -> Result<(TracerCliInitArgs, Pipeline)> {
        match self.command {
            DemoCommand::Fastquorum { init_args } => {
                let pipeline = Pipeline::select_demo_pipeline(Some("fastquorum".to_string()), init_args.interactive_prompts.clone())?;
                Ok((init_args, pipeline))
            }
            DemoCommand::Wdl { init_args } => {
                let pipeline = Pipeline::select_demo_pipeline(Some("wdl".to_string()), init_args.interactive_prompts.clone())?;
                Ok((init_args, pipeline))
            }
            DemoCommand::Run { name, init_args } => {
                let pipeline = Pipeline::select_demo_pipeline(Some(name), init_args.interactive_prompts.clone())?;
                Ok((init_args, pipeline))
            }
            DemoCommand::List => {
                // For list command, we don't need init args or pipeline selection
                // We'll handle this case in the handler
                Err(anyhow::anyhow!("List command should be handled separately"))
            }
        }
    }
}
