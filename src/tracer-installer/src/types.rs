use clap::{Parser, Subcommand};
use serde::Serialize;
use std::{collections::HashMap, str::FromStr};

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
        /// The channel or branch to install.
        /// Accepts "development", "production", or a custom branch name.
        #[arg(default_value = "development")]
        channel: TracerVersion,

        /// Optional user ID used to associate this installation with your account.
        #[arg(long)]
        user_id: Option<String>,
    },
}

#[derive(Serialize)]
pub struct AnalyticsPayload<'a> {
    #[serde(rename = "userId")]
    pub user_id: &'a str,
    pub event_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsEventType {
    InstallScriptStarted,
    InstallScriptCompleted,
}

impl AnalyticsEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnalyticsEventType::InstallScriptStarted => "install_script_started",
            AnalyticsEventType::InstallScriptCompleted => "install_script_completed",
        }
    }
}
