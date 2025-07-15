use crate::cli::handlers::arguments::TracerCliInitArgs;
use crate::utils::Version;
use clap::{Parser, Subcommand};

#[derive(Parser, Clone)]
#[clap(
    name = "tracer",
    about = "A tool for monitoring bioinformatics applications",
    version = Version::current_str()
)]
pub struct Cli {
    #[clap(long, global = true)]
    pub config: Option<String>,
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Start the daemon
    Init(Box<TracerCliInitArgs>),

    /// Stop the daemon
    Terminate,

    /// Remove all the temporary files created by the daemon, in a case of the process being terminated unexpectedly
    Cleanup,

    /// Shows the current configuration and the daemon status
    Info {
        /// Output information in JSON format
        #[clap(long)]
        json: bool,
    },

    /// Update the daemon to the latest version
    Update,

    /// Start a new pipeline run
    Start,

    /// End the current pipeline run
    End,

    /// Shows the current version of the daemon
    Version,
}
