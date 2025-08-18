use crate::process_identification::types::pipeline_tags::PipelineTags;
use crate::utils::input_validation::StringValueParser;
use clap::{Args, ValueEnum};
use serde::Serialize;
use std::collections::HashMap;

use super::resolver::ArgumentResolver;

pub const PIPELINE_NAME_ENV_VAR: &str = "TRACER_PIPELINE_NAME";
pub const RUN_NAME_ENV_VAR: &str = "TRACER_RUN_NAME";
pub const LOG_LEVEL_ENV_VAR: &str = "TRACER_LOG_LEVEL";
pub const USERNAME_ENV_VAR: &str = "USER";

#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliInitArgs {
    /// the name of the pipeline you will run; all pipelines with the same name are
    /// grouped together in the Tracer dashboard
    #[clap(short = 'p', long, value_parser = StringValueParser, env = PIPELINE_NAME_ENV_VAR)]
    pub pipeline_name: Option<String>,

    /// a unique name for this run that will be displayed in the UI; if not specified,
    /// a run name will be generated for you
    #[clap(long, value_parser = StringValueParser, env = RUN_NAME_ENV_VAR)]
    pub run_name: Option<String>,

    #[clap(flatten)]
    pub tags: PipelineTags,

    /// whether to prompt for missing inputs; if set to 'none', the client will exit with an error
    /// if any required inputs are missing
    #[clap(short = 'i', long, default_value = "minimal")]
    pub interactive_prompts: PromptMode,

    /// force process polling even if eBPF is available; this enables you to use
    /// the client without having root/sudo privileges
    #[clap(long)]
    pub force_procfs: bool,

    /// write log messages at the specified level and above to the daemon.log file;
    /// valid values: trace, debug, info, warn, error (default: info)
    #[clap(long, env = LOG_LEVEL_ENV_VAR, default_value = "info")]
    pub log_level: String,

    /// Additional environment variables for OpenTelemetry collector in KEY=VALUE format
    /// Can be specified multiple times (e.g: --env-var AWS_REGION=us-east-1 --env-var LOG_LEVEL=debug)
    #[clap(long, value_name = "KEY=VALUE")]
    pub env_var: Vec<String>,

    /// Directory to watch for log files (default: current working directory)
    #[clap(long, value_name = "DIR")]
    pub watch_dir: Option<String>,

    // run client as a standalone process rather than a daemon
    #[clap(long, hide = true)]
    pub no_daemonize: bool,

    // for testing purposes only
    #[clap(long, hide = true)]
    pub dev: bool,

    /// force termination of existing daemon before starting new one
    #[clap(long)]
    pub force: bool,
}

#[derive(Debug, Default, Clone, PartialEq, ValueEnum)]
pub enum PromptMode {
    /// do not prompt (i.e. non-interactive)
    None,
    /// only prompt for minimal information - automatically generate missing values when possible
    #[default]
    Minimal,
    /// prompt for all required values
    Required,
    // /// prompt for all values
    // All,
}

/// Ensures the pipeline name remains required
#[derive(Debug, Clone, Serialize)]
pub struct FinalizedInitArgs {
    pub pipeline_name: String,
    pub run_name: Option<String>,
    /// This is the same user_id as in tags, but is not optional
    pub user_id: String,
    pub tags: PipelineTags,
    pub no_daemonize: bool,
    pub dev: bool,
    pub force_procfs: bool,
    pub force: bool,
    pub log_level: String,
    pub environment_variables: HashMap<String, String>,
    pub watch_dir: Option<String>,
}

impl TracerCliInitArgs {
    /// Fill in any missing arguments according to the `PromptMode`.
    pub async fn resolve_arguments(self) -> FinalizedInitArgs {
        ArgumentResolver::new(self).resolve().await
    }

    /// Set the prompt mode to non-interactive (no prompts)
    pub fn set_non_interactive(&mut self) {
        self.interactive_prompts = PromptMode::None;
    }

    /// Set the prompt mode to minimal (auto-generate missing values when possible)
    pub fn set_minimal_prompts(&mut self) {
        self.interactive_prompts = PromptMode::Minimal;
    }

    /// Set the prompt mode to required (prompt for all required values)
    pub fn set_required_prompts(&mut self) {
        self.interactive_prompts = PromptMode::Required;
    }

    /// Configure init args for test scenarios with appropriate defaults
    pub fn configure_for_test(&mut self) {
        // Set test-specific watch directory
        if self.watch_dir.is_none() {
            self.watch_dir = Some("/tmp/tracer".to_string());
        }

        // Force non-interactive mode for tests
        self.set_non_interactive();
    }
}
