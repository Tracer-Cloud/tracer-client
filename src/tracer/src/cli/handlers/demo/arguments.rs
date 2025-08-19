use crate::cli::handlers::demo::pipeline::Pipeline;
use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::process_identification::types::pipeline_tags::PipelineTags;
use anyhow::Result;
use clap::{Args, Subcommand};

/// Run fastquorum or wdl demo pipelines to test out the Tracer client
#[derive(Args, Debug, Clone)]
#[command(help_template = r#"
Run fastquorum or wdl demo pipelines to test out the Tracer client

Usage: tracer demo [DEMO_PIPELINE_ID]       # DEMO_PIPELINE_ID = name of the demo pipeline

Examples:
  tracer demo                               # defaults to fastquorum
  tracer demo fastquorum                    # run the fastquorum demo pipeline
  tracer demo wdl                           # run the wdl demo pipeline
  tracer demo fastquorum --run-name test123 # run with custom run name

Help:
  -h, --help            Show this help message
      --help-advanced   Show all advanced and metadata options

Common options:
  -p, --pipeline-name   Logical name for grouping runs in the dashboard
      --run-name        Unique name for this run (auto-generated if omitted)
  -e, --environment     Execution context [ci-cd|sandbox|local]
      --instance-type   Cloud compute type (e.g., m5.large)
  -i, --interactive     Input prompts: [none|minimal|required] (default: minimal)

"#)]
pub struct TracerCliDemoArgs {
    /// Name of the demo pipeline to run (optional, defaults to fastquorum)
    pub demo_pipeline_id: Option<String>,

    #[command(flatten)]
    pub init_args: DemoInitArgs,

    /// Show all advanced and metadata options
    #[clap(long)]
    pub help_advanced: bool,
}

// We no longer need the DemoCommand enum since we're handling pipeline selection directly
// Keep it minimal for potential future use
#[derive(Subcommand, Debug, Clone)]
pub enum DemoCommand {
    /// List available demo pipelines
    List,
}

/// Common demo options (shown in default help)
#[derive(Args, Debug, Clone, Default)]
pub struct DemoInitArgs {
    /// Logical name for grouping runs in the dashboard
    #[clap(short = 'p', long)]
    pub pipeline_name: Option<String>,

    /// Unique name for this run (auto-generated if omitted)
    #[clap(long)]
    pub run_name: Option<String>,

    /// Execution context [ci-cd|sandbox|local]
    #[clap(short = 'e', long)]
    pub environment: Option<String>,

    /// Cloud compute type (e.g., m5.large)
    #[clap(long)]
    pub instance_type: Option<String>,

    /// Input prompts: [none|minimal|required] (default: minimal)
    #[clap(short = 'i', long, default_value = "minimal")]
    pub interactive: crate::cli::handlers::init::arguments::PromptMode,
}

impl DemoInitArgs {
    /// Convert to full TracerCliInitArgs with all options
    pub fn to_full_init_args(self) -> TracerCliInitArgs {
        TracerCliInitArgs {
            pipeline_name: self.pipeline_name,
            run_name: self.run_name,
            interactive_prompts: self.interactive,
            tags: PipelineTags {
                environment: self.environment,
                instance_type: self.instance_type,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

impl TracerCliDemoArgs {
    /// Resolve init arguments and select demo pipeline
    pub fn resolve_demo_arguments(self) -> Result<(TracerCliInitArgs, Pipeline)> {
        // Convert demo init args to full init args
        let full_init_args = self.init_args.to_full_init_args();

        // Determine which pipeline to run
        let pipeline_name = self
            .demo_pipeline_id
            .unwrap_or_else(|| "fastquorum".to_string());

        let pipeline = Pipeline::select_demo_pipeline(
            Some(pipeline_name),
            full_init_args.interactive_prompts.clone(),
        )?;

        Ok((full_init_args, pipeline))
    }

    /// Check if this is a list command (for special handling)
    pub fn is_list_command(&self) -> bool {
        // Since we removed the command structure, we need a different way to handle list
        // For now, we'll check if the pipeline_id is "list"
        self.demo_pipeline_id.as_deref() == Some("list")
    }
}
