use crate::cli::handlers::INTERACTIVE_THEME;
use crate::process_identification::types::pipeline_tags::PipelineTags;
use crate::utils::env;
use clap::Args;
use dialoguer::{Input, Select};
use serde::Serialize;

#[derive(Default, Args, Debug, Clone)]
pub struct TracerCliInitArgs {
    /// pipeline name to init the daemon with
    #[clap(long, short)]
    pub pipeline_name: Option<String>,

    // deprecated
    #[clap(long, hide = true)]
    pub run_id: Option<String>,

    // a unique name for this run that will be displayed in the UI
    #[clap(long)]
    pub run_name: Option<String>,

    #[clap(flatten)]
    pub tags: PipelineTags,

    /// Optional user ID used to associate this installation with your account.
    #[arg(long)]
    pub user_id: Option<String>,

    /// Run agent as a standalone process rather than a daemon
    #[clap(long)]
    pub no_daemonize: bool,

    /// Do not prompt for missing inputs
    #[clap(short = 'f', long)]
    pub non_interactive: bool,

    // For testing purposes only
    #[clap(long, hide = true)]
    pub is_dev: Option<bool>,
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

        let arg_pipeline_name = self
            .pipeline_name
            .or_else(|| env::get_env_var(env::PIPELINE_NAME_ENV_VAR));
        let pipeline_name = match (arg_pipeline_name, &prompt_mode) {
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => Some(name),
            (Some(name), PromptMode::Always) => Self::prompt_for_pipeline_name(&name),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Self::prompt_for_pipeline_name("demo_pipeline")
            }
            (None, PromptMode::Never) => None,
        }
        .expect("Failed to get pipeline name from environment variable or prompt");

        // Ignore empty run names
        let run_name = self
            .run_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .or_else(|| env::get_env_var(env::RUN_NAME_ENV_VAR));

        let mut tags = self.tags;

        let arg_environment = tags
            .environment
            .or_else(|| env::get_env_var(env::ENVIRONMENT_ENV_VAR));
        let environment = match (arg_environment, &prompt_mode) {
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => Some(name),
            (Some(name), PromptMode::Always) => Self::prompt_for_environment_name(&name),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Self::prompt_for_environment_name("local")
            }
            (None, PromptMode::Never) => None,
        }
        .expect("Failed to get environment from environment variable or prompt");
        tags.environment = Some(environment);

        let arg_pipeline_type = tags
            .pipeline_type
            .map(|e| e.clone())
            .or_else(|| env::get_env_var(env::PIPELINE_TYPE_ENV_VAR));
        let pipeline_type = match (arg_pipeline_type, &prompt_mode) {
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => Some(name),
            (Some(name), PromptMode::Always) => Self::prompt_for_pipeline_type(&name),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Self::prompt_for_pipeline_type("RNA-seq")
            }
            (None, PromptMode::Never) => None,
        }
        .expect("Failed to get pipeline type from environment variable or prompt");
        tags.pipeline_type = Some(pipeline_type);

        let arg_user_operator = tags
            .user_operator
            .or_else(|| env::get_env_var(env::USER_OPERATOR_ENV_VAR));
        let user_operator = match (arg_user_operator, &prompt_mode) {
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => Some(name),
            (Some(name), PromptMode::Always) => Self::prompt_for_user_operator(&name),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Self::prompt_for_user_operator("unknown")
            }
            (None, PromptMode::Never) => None,
        }
        .expect("Failed to get user operator from environment variable or prompt");
        tags.user_operator = Some(user_operator);

        FinalizedInitArgs {
            pipeline_name,
            run_id: self.run_id,
            run_name,
            tags,
            no_daemonize: self.no_daemonize,
            is_dev: self.is_dev,
            user_id: self.user_id,
        }
    }

    fn prompt_for_pipeline_name(default: &str) -> Option<String> {
        Input::with_theme(&*INTERACTIVE_THEME)
            .with_prompt("Enter pipeline name (e.g., RNA-seq_analysis_v1, scRNA-seq_2024)")
            .default(default.into())
            .interact_text()
            .inspect_err(|e| panic!("Error while prompting for pipeline type: {e}"))
            .ok()
    }

    fn prompt_for_environment_name(default: &str) -> Option<String> {
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
            Input::with_theme(&*INTERACTIVE_THEME)
                .with_prompt("Enter custom environment name")
                .interact_text()
                .ok()
        } else {
            Some(environment.to_string())
        }
    }

    fn prompt_for_pipeline_type(default: &str) -> Option<String> {
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
            let mut prompt =
                Input::with_theme(&*INTERACTIVE_THEME).with_prompt("Enter custom pipeline type");
            if default_index == CUSTOM_INDEX {
                prompt = prompt.default(default.into());
            }
            Some(
                prompt
                    .interact_text()
                    .expect("Error while prompting for pipeline type"),
            )
        } else {
            Some(pipeline_type.to_string())
        }
    }

    fn prompt_for_user_operator(default: &str) -> Option<String> {
        Input::with_theme(&*INTERACTIVE_THEME)
            .with_prompt("Enter your name/username (who is running this pipeline)")
            .default(std::env::var("USER").unwrap_or_else(|_| default.into()))
            .interact_text()
            .inspect_err(|e| panic!("Error while prompting for user operator: {e}"))
            .ok()
    }
}

/// Ensures the pipeline name remains required
#[derive(Debug, Clone, Serialize)]
pub struct FinalizedInitArgs {
    pub pipeline_name: String,
    pub run_id: Option<String>,
    pub run_name: Option<String>,
    pub tags: PipelineTags,
    pub no_daemonize: bool,
    pub is_dev: Option<bool>,
    pub user_id: Option<String>,
}
