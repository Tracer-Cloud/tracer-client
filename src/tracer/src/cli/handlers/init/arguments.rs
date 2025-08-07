use crate::cli::handlers::INTERACTIVE_THEME;
use crate::process_identification::types::pipeline_tags::PipelineTags;
use crate::utils::input_validation::{get_validated_input, StringValueParser};
use clap::Args;
use dialoguer::Select;
use serde::Serialize;

pub const PIPELINE_NAME_ENV_VAR: &str = "TRACER_PIPELINE_NAME";
pub const RUN_NAME_ENV_VAR: &str = "TRACER_RUN_NAME";
pub const LOG_LEVEL_ENV_VAR: &str = "TRACER_LOG_LEVEL";

#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliInitArgs {
    /// the name of the pipeline you will run; all pipelines with the same name are
    /// grouped together in the Tracer dashboard
    #[clap(long, short, value_parser = StringValueParser, env = PIPELINE_NAME_ENV_VAR)]
    pub pipeline_name: Option<String>,

    /// a unique name for this run that will be displayed in the UI; if not specified,
    /// a run name will be generated for you
    #[clap(long, value_parser = StringValueParser, env = RUN_NAME_ENV_VAR)]
    pub run_name: Option<String>,

    #[clap(flatten)]
    pub tags: PipelineTags,

    /// do not prompt for missing inputs; the client will exit with an error if any
    /// required inputs are missing
    #[clap(short = 'f', long)]
    pub non_interactive: bool,

    /// force process polling even if eBPF is available; this enables you to use
    /// the client without having root/sudo privileges
    #[clap(long)]
    pub force_procfs: bool,

    /// write log messages at the specified level and above to the daemon.log file;
    /// valid values: trace, debug, info, warn, error (default: info)
    #[clap(long, env = LOG_LEVEL_ENV_VAR, default_value = "info")]
    pub log_level: String,

    // run client as a standalone process rather than a daemon
    #[clap(long, hide = true)]
    pub no_daemonize: bool,

    // for testing purposes only
    #[clap(long, hide = true)]
    pub dev: bool,
}

pub enum PromptMode {
    Always,
    Never,
    WhenMissing,
}

impl TracerCliInitArgs {
    pub fn finalize(self, default_prompt_mode: PromptMode) -> FinalizedInitArgs {
        let prompt_mode = if self.non_interactive {
            PromptMode::Never
        } else {
            default_prompt_mode
        };

        // Validate pipeline name
        let pipeline_name = match (self.pipeline_name, &prompt_mode) {
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => Some(name),
            (Some(name), PromptMode::Always) => Some(Self::prompt_for_pipeline_name(&name)),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Some(Self::prompt_for_pipeline_name("demo_pipeline"))
            }
            (None, PromptMode::Never) => None,
        }
        .or_else(print_help)
        .expect("Failed to get pipeline name from command line, environment variable, or prompt");

        // Ignore empty run names
        let run_name = self
            .run_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());

        let mut tags = self.tags;

        let environment = match (tags.environment, &prompt_mode) {
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => Some(name),
            (Some(name), PromptMode::Always) => Some(Self::prompt_for_environment_name(&name)),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Some(Self::prompt_for_environment_name("local"))
            }
            (None, PromptMode::Never) => None,
        }
        .or_else(print_help)
        .expect("Failed to get environment from command line, environment variable, or prompt");
        tags.environment = Some(environment);

        let pipeline_type = match (tags.pipeline_type, &prompt_mode) {
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => Some(name),
            (Some(name), PromptMode::Always) => Some(Self::prompt_for_pipeline_type(&name)),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Some(Self::prompt_for_pipeline_type("RNA-seq"))
            }
            (None, PromptMode::Never) => None,
        }
        .or_else(print_help)
        .expect("Failed to get pipeline type from command line, environment variable, or prompt");
        tags.pipeline_type = Some(pipeline_type);

        // First try to get user_id from the tags, then from the environment variable, then prompt
        let user_id = match (tags.user_id, prompt_mode) {
            (Some(user_id), PromptMode::Never | PromptMode::WhenMissing) => Some(user_id),
            (Some(user_id), PromptMode::Always) => Some(Self::prompt_for_user_id(Some(&user_id))),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Some(Self::prompt_for_user_id(None))
            }
            (None, PromptMode::Never) => None,
        }
        .or_else(print_help)
        .expect("Failed to get user ID from environment variable, command line, or prompt");
        tags.user_id = Some(user_id.clone());

        FinalizedInitArgs {
            pipeline_name,
            run_name,
            tags,
            no_daemonize: self.no_daemonize,
            dev: self.dev,
            force_procfs: self.force_procfs,
            user_id,
            log_level: self.log_level,
        }
    }

    fn prompt_for_pipeline_name(default: &str) -> String {
        get_validated_input(
            &INTERACTIVE_THEME,
            "Enter pipeline name (e.g., RNA-seq_analysis_v1, scRNA-seq_2024)",
            Some(default),
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
            "RNA-seq",
            "scRNA-seq",
            "ChIP-seq",
            "ATAC-seq",
            "WGS",
            "WES",
            "Metabolomics",
            "Proteomics",
            "custom",
        ];
        const CUSTOM_INDEX: usize = 8;
        let default_index = PIPELINE_TYPES
            .iter()
            .position(|e| e == &default)
            .unwrap_or(CUSTOM_INDEX);
        let selection = Select::with_theme(&*INTERACTIVE_THEME)
            .with_prompt("Select pipeline type (or choose 'custom' to enter your own)")
            .items(PIPELINE_TYPES)
            .default(default_index)
            .interact()
            .expect("Error while prompting for pipeline type");
        let pipeline_type = PIPELINE_TYPES[selection];
        if pipeline_type == "custom" {
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
    variables. Required parameters are denoted by (*).

    Parameter       | Command Line Option | Environment Variable
    ----------------|---------------------|-----------------------
    pipeline_name*  | --pipeline-name     | TRACER_PIPELINE_NAME
    pipeline_type*  | --pipeline-type     | TRACER_PIPELINE_TYPE
    environment*    | --environment       | TRACER_ENVIRONMENT
    user_id*        | --user-id           | TRACER_USER_ID
    run_name        | --run-name          | TRACER_RUN_NAME
    department      | --department        | TRACER_DEPARTMENT
    team            | --team              | TRACER_TEAM
    organization_id | --organization-id   | TRACER_ORGANIZATION_ID
    "#
    );
    None::<T>
}

/// Ensures the pipeline name remains required
#[derive(Debug, Clone, Serialize)]
pub struct FinalizedInitArgs {
    pub pipeline_name: String,
    pub run_name: Option<String>,
    pub tags: PipelineTags,
    pub no_daemonize: bool,
    pub dev: bool,
    pub force_procfs: bool,
    pub user_id: String,
    pub log_level: String,
}
