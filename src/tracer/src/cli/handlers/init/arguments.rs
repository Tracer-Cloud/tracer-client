use crate::process_identification::types::pipeline_tags::PipelineTags;
use crate::utils::env;
use crate::utils::input_validation::{get_validated_input, validate_input_string};
use clap::Args;
use console::Emoji;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use serde::Serialize;
use std::sync::LazyLock;

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

impl TracerCliInitArgs {
    pub fn finalize(self) -> FinalizedInitArgs {
        let theme: LazyLock<ColorfulTheme> = LazyLock::new(|| {
            let arrow = Emoji("👉 ", "> ").to_string();
            ColorfulTheme {
                prompt_prefix: dialoguer::console::Style::new().green().apply_to(arrow),
                prompt_suffix: dialoguer::console::Style::new()
                    .dim()
                    .apply_to(":".to_string()),
                success_prefix: dialoguer::console::Style::new()
                    .green()
                    .apply_to("✔".to_string()),
                success_suffix: dialoguer::console::Style::new()
                    .dim()
                    .apply_to("".to_string()),
                values_style: dialoguer::console::Style::new().yellow(),
                active_item_style: dialoguer::console::Style::new().cyan().bold(),
                ..ColorfulTheme::default()
            }
        });

        let pipeline_name = self
            .pipeline_name
            .or_else(|| env::get_env_var(env::PIPELINE_NAME_ENV_VAR))
            .or_else(|| {
                if self.non_interactive {
                    None
                } else {
                    Some(get_validated_input(
                        &*theme,
                        "Enter pipeline name (e.g., RNA-seq_analysis_v1, scRNA-seq_2024)",
                        Some("demo_pipeline".into()),
                        "pipeline name",
                    ))
                }
            })
            .expect("Failed to get pipeline name from environment variable or prompt");

        // Validate pipeline name
        if let Err(e) = validate_input_string(&pipeline_name, "pipeline name") {
            panic!("Invalid pipeline name: {}", e);
        }

        // Ignore empty run names
        let run_name = self
            .run_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .or_else(|| env::get_env_var(env::RUN_NAME_ENV_VAR))
            .map(|name| {
                if let Err(e) = validate_input_string(&name, "run name") {
                    panic!("Invalid run name: {}", e);
                }
                name
            });

        let mut tags = self.tags;

        if tags.environment.is_none() {
            let environment = env::get_env_var(env::ENVIRONMENT_ENV_VAR)
                .or_else(|| {
                    if self.non_interactive {
                        None
                    } else {
                        const ENVIRONMENTS: &[&str] =
                            &["local", "development", "staging", "production", "custom"];
                        let selection = Select::with_theme(&*theme)
                            .with_prompt(
                                "Select environment (or choose 'custom' to enter your own)",
                            )
                            .items(ENVIRONMENTS)
                            .default(0)
                            .interact()
                            .expect("Error while prompting for environment name");
                        if selection == 4 {
                            Some(get_validated_input(
                                &*theme,
                                "Enter custom environment name",
                                None,
                                "environment name",
                            ))
                        } else {
                            Some(ENVIRONMENTS[selection].to_string())
                        }
                    }
                })
                .expect("Failed to get environment from environment variable or prompt");

            // Validate environment
            if let Err(e) = validate_input_string(&environment, "environment") {
                panic!("Invalid environment: {}", e);
            }

            tags.environment = Some(environment);
        }

        if tags.pipeline_type.is_none() {
            let pipeline_type = env::get_env_var(env::PIPELINE_TYPE_ENV_VAR)
                .or_else(|| {
                    if self.non_interactive {
                        None
                    } else {
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
                        let selection = Select::with_theme(&*theme)
                            .with_prompt(
                                "Select pipeline type (or choose 'custom' to enter your own)",
                            )
                            .items(PIPELINE_TYPES)
                            .default(0)
                            .interact()
                            .expect("Error while prompting for pipeline type");

                        if selection == 8 {
                            Some(get_validated_input(
                                &*theme,
                                "Enter custom pipeline type",
                                None,
                                "pipeline type",
                            ))
                        } else {
                            Some(PIPELINE_TYPES[selection].to_string())
                        }
                    }
                })
                .expect("Failed to get pipeline type from environment variable or prompt");

            // Validate pipeline type
            if let Err(e) = validate_input_string(&pipeline_type, "pipeline type") {
                panic!("Invalid pipeline type: {}", e);
            }

            tags.pipeline_type = Some(pipeline_type);
        }
        if tags.user_operator.is_none() {
            let user_operator = env::get_env_var(env::USER_OPERATOR_ENV_VAR)
                .or_else(|| {
                    if self.non_interactive {
                        None
                    } else {
                        Some(get_validated_input(
                            &*theme,
                            "Enter your API key",
                            None,
                            "API key",
                        ))
                    }
                })
                .expect("Failed to get API key from environment variable or prompt");

            // Validate API key
            if let Err(e) = validate_input_string(&user_operator, "API key") {
                panic!("Invalid API key: {}", e);
            }

            tags.user_operator = Some(user_operator);
        }

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
        let user_id = self.user_id.map(|id| {
            if let Err(e) = validate_input_string(&id, "user_id") {
                panic!("Invalid user_id: {}", e);
            }
            id
        });

        // Validate run_id if provided
        let run_id = self.run_id.map(|id| {
            if let Err(e) = validate_input_string(&id, "run_id") {
                panic!("Invalid run_id: {}", e);
            }
            id
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
