use crate::cli::handlers::INTERACTIVE_THEME;
use crate::process_identification::types::pipeline_tags::PipelineTags;
use crate::utils::env;
use crate::utils::input_validation::{get_validated_input, validate_input_string};
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
    pub dev: bool,

    /// Force process polling when eBPF is not available
    #[clap(long)]
    pub force_procfs: bool,

    /// Capture logs at the specified level and above (default: info)
    /// Valid values: trace, debug, info, warn, error
    /// Output will be written to `daemon.log` in the working directory.
    #[clap(long, default_value = "info")]
    pub log_level: String,
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
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => {
                if let Err(e) = validate_input_string(&name, "pipeline name") {
                    panic!("Invalid pipeline name: {}", e);
                }
                Some(name)
            }
            (Some(name), PromptMode::Always) => Some(Self::prompt_for_pipeline_name(&name)),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Some(Self::prompt_for_pipeline_name("demo_pipeline"))
            }
            (None, PromptMode::Never) => None,
        }
        .expect("Failed to get pipeline name from environment variable or prompt");

        // Validate pipeline name

        // Ignore empty run names
        let run_name = self
            .run_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .or_else(|| env::get_env_var(env::RUN_NAME_ENV_VAR))
            .inspect(|name| {
                if let Err(e) = validate_input_string(name, "run name") {
                    panic!("Invalid run name: {}", e);
                }
            });

        let mut tags = self.tags;

        let arg_environment = tags
            .environment
            .or_else(|| env::get_env_var(env::ENVIRONMENT_ENV_VAR));
        let environment = match (arg_environment, &prompt_mode) {
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => {
                if let Err(e) = validate_input_string(&name, "environment name") {
                    panic!("Invalid environment name: {}", e);
                }
                Some(name)
            }
            (Some(name), PromptMode::Always) => Some(Self::prompt_for_environment_name(&name)),
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Some(Self::prompt_for_environment_name("local"))
            }
            (None, PromptMode::Never) => None,
        }
        .expect("Failed to get environment from environment variable or prompt");
        tags.environment = Some(environment);

        let arg_pipeline_type = tags
            .pipeline_type
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
            (Some(name), PromptMode::Never | PromptMode::WhenMissing) => {
                if let Err(e) = validate_input_string(&name, "user operator") {
                    panic!("Invalid API Key: {}", e);
                }
                Some(name)
            }
            (Some(name), PromptMode::Always) => {
                Some(Self::prompt_for_api_key(Some(name.to_owned())))
            }
            (None, PromptMode::Always | PromptMode::WhenMissing) => {
                Some(Self::prompt_for_api_key(None))
            }
            (None, PromptMode::Never) => None,
        }
        .expect("Failed to get user operator from environment variable or prompt");
        tags.user_operator = Some(user_operator);

        // Validate department
        if let Err(e) = validate_input_string(&tags.department, "department") {
            panic!("Invalid department: {}", e);
        }

        // Validate team
        if let Err(e) = validate_input_string(&tags.team, "team") {
            panic!("Invalid team: {}", e);
        }

        // Validate organization_id if provided
        if let Some(ref org_id) = tags.organization_id {
            if let Err(e) = validate_input_string(org_id, "organization_id") {
                panic!("Invalid organization_id: {}", e);
            }
        }

        // Validate others tags
        for (i, other_tag) in tags.others.iter().enumerate() {
            if let Err(e) = validate_input_string(other_tag, &format!("others[{}]", i)) {
                panic!("Invalid others tag at index {}: {}", i, e);
            }
        }

        // Validate user_id if provided
        let user_id = self.user_id.inspect(|id| {
            if let Err(e) = validate_input_string(id, "user_id") {
                panic!("Invalid user_id: {}", e);
            }
        });

        // Validate run_id if provided
        let run_id = self.run_id.inspect(|id| {
            if let Err(e) = validate_input_string(id, "run_id") {
                panic!("Invalid run_id: {}", e);
            }
        });

        // Validate log_level
        if let Err(e) = validate_input_string(&self.log_level, "log_level") {
            panic!("Invalid log_level: {}", e);
        }

        FinalizedInitArgs {
            pipeline_name,
            run_id,
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
            Some(default.into()),
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

    fn prompt_for_api_key(default: Option<String>) -> String {
        get_validated_input(&INTERACTIVE_THEME, "Enter your API key", default, "API key")
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
    pub dev: bool,
    pub force_procfs: bool,
    pub user_id: Option<String>,
    pub log_level: String,
}
