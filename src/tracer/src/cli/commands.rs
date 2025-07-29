use crate::cli::handlers::init_arguments::TracerCliInitArgs;
use crate::cli::handlers::test_arguments::TracerCliTestArgs;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::utils::Version;
use clap::{Parser, Subcommand};

fn about_message() -> String {
    format!(
        "A tool for monitoring bioinformatics applications\nVersion: {}",
        Version::current_str()
    )
}

fn footer_message() -> String {
    format!(
        "Working Directory: {:?}\nDaemon stdout: {:?}\nDaemon stderr: {:?}Daemon log: {:?}\nFor more information, visit: https://tracer.cloud\n",
        &TRACER_WORK_DIR.path,
        &TRACER_WORK_DIR.stdout_file,
        &TRACER_WORK_DIR.stderr_file,
        &TRACER_WORK_DIR.log_file
    )
}

#[derive(Parser, Clone)]
#[clap(
    name = "tracer",
    about = about_message(),
    version = Version::current_str(),
    after_help = footer_message()
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

    /// Free up the port used by the daemon in case of an issue with it being unresponsive.
    CleanupPort,

    /// Shows the current configuration and the daemon status
    Info {
        /// Output information in JSON format
        #[clap(long)]
        json: bool,
    },

    /// Execute example pipelines
    Test(TracerCliTestArgs),

    /// Update the daemon to the latest version
    Update,

    /// Start a new pipeline run
    Start,

    /// End the current pipeline run
    End,

    /// Shows the current version of the daemon
    Version,

    /// Uninstall tracer
    Uninstall,
}
