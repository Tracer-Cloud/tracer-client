use crate::common::types::pipeline_tags::PipelineTags;
use clap::Args;

#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliInitArgs {
    // todo: move to tracer_cli!
    /// pipeline name to init the daemon with
    #[clap(long, short)]
    pub pipeline_name: Option<String>,

    /// Run Identifier: this is used group same pipeline runs on different computers.
    /// Context: types batch can run same pipeline on multiple machines for speed
    #[clap(long)]
    pub run_id: Option<String>,

    #[clap(flatten)]
    pub tags: PipelineTags,

    /// Run agent as a standalone process rather than a daemon
    #[clap(long)]
    pub no_daemonize: bool,

    #[clap(long)]
    pub is_dev: Option<bool>,
}

/// Ensures the pipeline name remains required
#[derive(Debug, Clone)]
pub struct FinalizedInitArgs {
    pub pipeline_name: String,
    pub run_id: Option<String>,
    pub tags: PipelineTags,
    pub no_daemonize: bool,
    pub is_dev: Option<bool>,
}
