use crate::common::types::cli::params::TracerCliInitArgs;
use clap::{Parser, Subcommand};

#[derive(Parser, Clone)]
#[clap(
    name = "tracer",
    about = "A tool for monitoring bioinformatics applications",
    version = env!("CARGO_PKG_VERSION")
)]
pub struct Cli {
    #[clap(long, global = true)]
    pub config: Option<String>,
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Setup the configuration for the service, rewriting the config.toml file
    Setup {
        /// API key for the service
        #[clap(long, short)]
        api_key: Option<String>,
        /// Interval in milliseconds for polling process information
        #[clap(long, short)]
        process_polling_interval_ms: Option<u64>,
        /// Interval in milliseconds for submitting batch data
        #[clap(long, short)]
        batch_submission_interval_ms: Option<u64>,
    },

    /// Log a message to the service
    Log { message: String },

    /// Send an alert to the service, sending an e-mail
    Alert { message: String },

    /// Start the daemon
    Init(TracerCliInitArgs),

    /// Stop the daemon
    Terminate,

    /// Remove all the temporary files created by the daemon, in a case of the process being terminated unexpectedly
    Cleanup,

    /// Shows the current configuration and the daemon status
    Info,

    /// Update the daemon to the latest version
    Update,

    /// Start a new pipeline run
    Start,

    /// End the current pipeline run
    End,

    /// Test the configuration by sending a request to the service
    Test,

    /// Upload a file to the service [Works only directly from the function not the daemon]
    Upload { file_path: String },

    /// Upload a file to the service [Works only directly from the function not the daemon]
    UploadDaemon,

    /// Change the tags of the current pipeline run
    Tag { tags: Vec<String> },

    /// Configure .bashrc file to include aliases for short-lived processes commands. To use them, a new terminal session must be started.
    ApplyBashrc,

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
