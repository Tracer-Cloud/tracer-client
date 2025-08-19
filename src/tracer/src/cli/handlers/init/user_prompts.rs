use crate::cli::handlers::INTERACTIVE_THEME;
use crate::utils::input_validation::get_validated_input;
use dialoguer::Select;

/// Constants for user prompting
pub const ENVIRONMENTS: &[&str] = &["local", "development", "staging", "production", "custom"];
pub const PIPELINE_TYPES: &[&str] = &[
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
pub const CUSTOM_INDEX: usize = 9;

/// Handles all user prompting functionality
pub struct UserPrompts;

impl UserPrompts {
    pub fn prompt_for_pipeline_name<S: AsRef<str>>(default: S) -> Option<String> {
        get_validated_input(
            &INTERACTIVE_THEME,
            "Enter pipeline name (e.g., RNA-seq_analysis_v1, scRNA-seq_2024)",
            Some(default.as_ref()),
            "pipeline name",
        )
    }

    pub fn prompt_for_environment_name(default: &str) -> String {
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
            .unwrap_or_else(|| "custom".to_string())
        } else {
            environment.to_string()
        }
    }

    pub fn prompt_for_pipeline_type(default: &str) -> String {
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
            .unwrap_or_else(|| "Custom".to_string())
        } else {
            pipeline_type.to_string()
        }
    }

    pub fn prompt_for_user_id(default: Option<&str>) -> Option<String> {
        get_validated_input(&INTERACTIVE_THEME, "Enter your User ID", default, "User ID")
    }
}

pub fn print_help<T>() -> Option<T> {
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
