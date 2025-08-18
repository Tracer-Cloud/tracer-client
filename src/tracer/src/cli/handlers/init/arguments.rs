use crate::cli::handlers::INTERACTIVE_THEME;
use crate::process_identification::types::pipeline_tags::PipelineTags;
use crate::utils::env;
use crate::utils::input_validation::{get_validated_input, StringValueParser};
use crate::warning_message;
use clap::{Args, ValueEnum};
use colored::Colorize;
use dialoguer::Select;
use serde::Serialize;
use std::collections::HashMap;

pub const PIPELINE_NAME_ENV_VAR: &str = "TRACER_PIPELINE_NAME";
pub const RUN_NAME_ENV_VAR: &str = "TRACER_RUN_NAME";
pub const LOG_LEVEL_ENV_VAR: &str = "TRACER_LOG_LEVEL";
pub const USERNAME_ENV_VAR: &str = "USER";

const DEFAULT_PIPELINE_TYPE: &str = "Preprocessing";
const DEFAULT_ENVIRONMENT: &str = "local";

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

impl TracerCliInitArgs {
    /// Fill in any missing arguments according to the `PromptMode`.
    pub async fn finalize(self, default_pipeline_prefix: &str) -> FinalizedInitArgs {
        let prompt_mode = if self.no_daemonize {
            PromptMode::None
        } else {
            self.interactive_prompts
        };
        let mut tags = self.tags;

        // user_id is required - try to get it from the command line, fall back to user prompt
        // unless mode is set to non-interactive; error if missing
        let username = env::get_env_var(USERNAME_ENV_VAR);
        let user_id = match (tags.user_id, &prompt_mode) {
            (Some(user_id), PromptMode::Required) => {
                // Only prompt for confirmation in Required mode
                Some(Self::prompt_for_user_id(Some(&user_id)))
            }
            (Some(user_id), _) => Some(user_id),
            (None, PromptMode::Minimal | PromptMode::Required) => {
                Some(Self::prompt_for_user_id(username.as_deref()))
            }
            (None, PromptMode::None)  => {
                // TODO: remove this once we can source the user ID from the credentials file
                if let Some(username) = &username {
                    warning_message!(
                        "Failed to get user ID from environment variable, command line, or prompt. \
                        defaulting to the system username '{}', which may not be your Tracer user ID! \
                        Please set the TRACER_USER_ID environment variable or specify the --user-id \
                        option.",
                        username
                    );
                }
                username
            }
        }
        .or_else(print_help)
        .expect("Failed to get user ID from environment variable, command line, or prompt");
        tags.user_id = Some(user_id.clone());

        // pipeline name is required - try to get it from the command line, fall back to user
        // prompt unless mode is set to non-interactive; generate from user-id if missing
        let pipeline_name = match (self.pipeline_name, &prompt_mode) {
            (Some(name), PromptMode::Required) => {
                // Only prompt for confirmation in Required mode
                Some(Self::prompt_for_pipeline_name(&name))
            }
            (Some(name), _) => Some(name),
            (None, PromptMode::Minimal | PromptMode::Required) => {
                Some(Self::prompt_for_pipeline_name(
                    Self::generate_pipeline_name(default_pipeline_prefix, &user_id),
                ))
            }
            (None, PromptMode::None) => Some(Self::generate_pipeline_name(
                default_pipeline_prefix,
                &user_id,
            )),
        }
        .or_else(print_help)
        .expect("Failed to get pipeline name from command line, environment variable, or prompt");

        // Ignore empty run names
        let run_name = self
            .run_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());

        // this call can take a while - if this is the daemon process being spawned, defer it until
        // we create the client, otherwise use a short timeout so the init call doesn't take too long
        if tags.environment_type.is_none() && !self.no_daemonize {
            tags.environment_type = Some(env::detect_environment_type(1).await);
        }

        // Environment is required but not included in minimal options - try to get it from the
        // command line, fall back to user prompt if the mode allows it, otherwise generate a
        // default value
        let environment = match (tags.environment, &prompt_mode) {
            (Some(env), PromptMode::Required) => Some(Self::prompt_for_environment_name(&env)),
            (Some(name), _) => Some(name),
            (None, PromptMode::Required) if tags.environment_type.is_some() => Some(
                Self::prompt_for_environment_name(tags.environment_type.as_ref().unwrap()),
            ),
            (None, PromptMode::Required) => {
                Some(Self::prompt_for_environment_name(DEFAULT_ENVIRONMENT))
            }
            (None, _) if tags.environment_type.is_some() => tags.environment_type.clone(),
            (None, _) => Some(DEFAULT_ENVIRONMENT.to_string()),
        }
        .or_else(print_help)
        .expect("Failed to get environment from command line, environment variable, or prompt");
        tags.environment = Some(environment);

        let pipeline_type = match (tags.pipeline_type, &prompt_mode) {
            (Some(env), PromptMode::Required) => Self::prompt_for_pipeline_type(&env),
            (Some(env), _) => env,
            (None, PromptMode::Required) => Self::prompt_for_pipeline_type(DEFAULT_PIPELINE_TYPE),
            (None, _) => DEFAULT_PIPELINE_TYPE.to_string(),
        };
        tags.pipeline_type = Some(pipeline_type);

        // Process environment variables
        let mut environment_variables = HashMap::new();
        for env_var in &self.env_var {
            if let Some((key, value)) = env_var.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() {
                    environment_variables.insert(key.to_string(), value.to_string());
                }
            }
        }

        FinalizedInitArgs {
            pipeline_name,
            run_name,
            user_id,
            tags,
            no_daemonize: self.no_daemonize,
            dev: self.dev,
            force_procfs: self.force_procfs,
            log_level: self.log_level,
            environment_variables,
            watch_dir: self.watch_dir,
        }
    }

    fn generate_pipeline_name(prefix: &str, user_id: &str) -> String {
        // TODO: use username instead? Either from UI (via API call) or from env::get_env("USER").
        format!("{}-{}", prefix, user_id)
    }

    fn prompt_for_pipeline_name<S: AsRef<str>>(default: S) -> String {
        get_validated_input(
            &INTERACTIVE_THEME,
            "Enter pipeline name (e.g., RNA-seq_analysis_v1, scRNA-seq_2024)",
            Some(default.as_ref()),
            "pipeline name",
        )
    }

    fn prompt_for_environment_name(default: &str) -> String {
        const ENVIRONMENTS: &[&str] = &["local", "development", "staging", "production", "custom"];
        let default_index = ENVIRONMENTS.iter().position(|e| e == &default).unwrap();
        let selection = Select::with_theme(&*INTERACTIVE_THEME)
            .with_prompt("Select environment (or choose 'custom' to enter your own)")
            .items(ENVIRONMENTS)
            .default(default_index)
            .interact()
            .expect("Error while prompting for environment name");
        let environment = ENVIRONMENTS[selection];
        if environment == "custom" {
            get_validated_input(
                &INTERACTIVE_THEME,
                "Enter custom environment name",
                None,
                "environment name",
            )
        } else {
            environment.to_string()
        }
    }

    fn prompt_for_pipeline_type(default: &str) -> String {
        const PIPELINE_TYPES: &[&str] = &[
            "Preprocessing",
            "RNA-seq",
            "scRNA-seq",
            "ChIP-seq",
            "ATAC-seq",
            "WGS",
            "WES",
            "Metabolomics",
            "Proteomics",
            "Custom",
        ];
        const CUSTOM_INDEX: usize = 9;
        let default_index = PIPELINE_TYPES
            .iter()
            .position(|e| e == &default)
            .unwrap_or(CUSTOM_INDEX);
        let selection = Select::with_theme(&*INTERACTIVE_THEME)
            .with_prompt("Select pipeline type (or choose 'Custom' to enter your own)")
            .items(PIPELINE_TYPES)
            .default(default_index)
            .interact()
            .expect("Error while prompting for pipeline type");
        let pipeline_type = PIPELINE_TYPES[selection];
        if pipeline_type.to_lowercase() == "custom" {
            let default = if default_index == CUSTOM_INDEX {
                Some(default)
            } else {
                None
            };
            get_validated_input(
                &INTERACTIVE_THEME,
                "Enter custom pipeline type",
                default,
                "pipeline type",
            )
        } else {
            pipeline_type.to_string()
        }
    }

    fn prompt_for_user_id(default: Option<&str>) -> String {
        get_validated_input(&INTERACTIVE_THEME, "Enter your User ID", default, "User ID")
    }
}

fn print_help<T>() -> Option<T> {
    println!(
        r#"
    The following parameters may be set interactively or with command-line options or environment
    variables. Required parameters that must be specified by the user are denoted by (*). Required
    parameters that will have auto-generated values by default are denoted by (**). Optional
    parameters that are auto-detected from the environment if unset are denoted by (***).

    Parameter           | Command Line Option | Environment Variable
    --------------------|---------------------|-----------------------
    user_id*            | --user-id           | TRACER_USER_ID
    pipeline_name**     | --pipeline-name     | TRACER_PIPELINE_NAME
    pipeline_type**     | --pipeline-type     | TRACER_PIPELINE_TYPE
    environment**       | --environment       | TRACER_ENVIRONMENT
    run_name            | --run-name          | TRACER_RUN_NAME
    department          | --department        | TRACER_DEPARTMENT
    team                | --team              | TRACER_TEAM
    organization_id     | --organization-id   | TRACER_ORGANIZATION_ID
    instance_type***    | --instance-type     | TRACER_INSTANCE_TYPE
    environment_type*** | --environment-type  | TRACER_ENVIRONMENT_TYPE
    
    OpenTelemetry Configuration:
    env_vars           | --env-var KEY=VALUE  | (multiple supported, interactive prompts available)
    watch_dir          | --watch-dir DIR      | (default: current working directory)

    "#
    );
    None::<T>
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
    pub log_level: String,
    pub environment_variables: HashMap<String, String>,
    pub watch_dir: Option<String>,
}
