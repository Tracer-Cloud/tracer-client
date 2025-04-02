use clap::Args;

#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliInitArgs {
    /// pipeline name to init the daemon with
    #[clap(long, short)]
    pub pipeline_name: String,

    /// Run Identifier: this is used group same pipeline runs on different computers.
    /// Context: aws batch can run same pipeline on multiple machines for speed
    #[clap(long)]
    pub run_id: Option<String>,

    /// attribution: used to assign tags to a certain pipeline
    #[clap(long, value_delimiter = ',')]
    pub tags: Vec<String>,

    /// Run agent as a standalone process rather than a daemon
    #[clap(long)]
    pub no_daemonize: bool,
}
