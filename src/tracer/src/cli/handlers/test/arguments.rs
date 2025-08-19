use clap::Args;

/// [DEPRECATED] Test command - use 'demo' instead
#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliTestArgs {
    /// [DEPRECATED] Use 'tracer demo' instead
    #[clap(short = 'd', long)]
    pub demo_pipeline_id: Option<String>,
}
