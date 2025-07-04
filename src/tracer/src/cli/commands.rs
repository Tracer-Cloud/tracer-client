use crate::process_identification::types::cli::params::TracerCliInitArgs;
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
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Log a message to the service
    Log { message: String },

    /// Send an alert to the service, sending an e-mail
    Alert { message: String },

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

    /// Upload a file to the service [Works only directly from the function not the daemon]
    Upload { file_path: String },

    /// Upload a file to the service [Works only directly from the function not the daemon]
    UploadDaemon,

    /// Change the tags of the current pipeline run
    Tag { tags: Vec<String> },

    /// Log a message to the service for a short-lived process.
    LogShortLivedProcess { command: String },

    /// Shows the current version of the daemon
    Version,

    /// Clean up port conflicts by finding and killing processes using the Tracer port
    CleanupPort {
        /// Port number to check and clean up (default: 8722)
        #[clap(long, short)]
        port: Option<u16>,
    },
}
