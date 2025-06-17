use clap::{Parser, Subcommand};
use std::str::FromStr;

#[derive(Clone, Debug)]
pub(crate) enum TracerVersion {
    Development,
    Production,
    Feature(String),
}

impl FromStr for TracerVersion {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "production" | "prod" => Ok(Self::Production),
            other => Ok(Self::Feature(other.to_string())),
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "tracer-installer", version, about = "Installs the Tracer CLI")]
pub struct InstallTracerCli {
    #[command(subcommand)]
    pub command: InstallerCommand,
}

#[derive(Subcommand, Debug)]
pub enum InstallerCommand {
    /// Run the Tracer installer with the specified version or branch
    Run {
        /// The version or branch to install.
        /// Accepts "development", "production", or a custom branch name.
        #[arg(default_value = "development")]
        version: TracerVersion,

        /// Optional user ID used to associate this installation with your account.
        #[arg(long)]
        user_id: Option<String>,
    },
}
