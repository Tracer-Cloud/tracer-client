use crate::process_identification::types::pipeline_tags::PipelineTags;
use crate::utils::env;
use crate::utils::env::get_user_id;
use clap::Args;
use console::Emoji;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};
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
    pub is_dev: Option<bool>,
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
                    Input::with_theme(&*theme)
                        .with_prompt(
                            "Enter pipeline name (e.g., RNA-seq_analysis_v1, scRNA-seq_2024)",
                        )
                        .default("demo_pipeline".into())
                        .interact_text()
                        .inspect_err(|e| panic!("Error while prompting for pipeline type: {e}"))
                        .ok()
                }
            })
            .expect("Failed to get pipeline name from environment variable or prompt");

        // Ignore empty run names
        let run_name = self
            .run_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .or_else(|| env::get_env_var(env::RUN_NAME_ENV_VAR));

        let mut tags = self.tags;

        if tags.environment.is_none() {
            let _ = tags.environment.insert(
                env::get_env_var(env::ENVIRONMENT_ENV_VAR)
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
                                Some(
                                    Input::with_theme(&*theme)
                                        .with_prompt("Enter custom environment name")
                                        .interact_text()
                                        .expect("Error while prompting for environment name"),
                                )
                            } else {
                                Some(ENVIRONMENTS[selection].to_string())
                            }
                        }
                    })
                    .expect("Failed to get environment from environment variable or prompt"),
            );
        }

        if tags.pipeline_type.is_none() {
            let _ = tags.pipeline_type.insert(
                env::get_env_var(env::PIPELINE_TYPE_ENV_VAR)
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
                                Some(
                                    Input::with_theme(&*theme)
                                        .with_prompt("Enter custom pipeline type")
                                        .interact_text()
                                        .expect("Error while prompting for pipeline type"),
                                )
                            } else {
                                Some(PIPELINE_TYPES[selection].to_string())
                            }
                        }
                    })
                    .expect("Failed to get pipeline type from environment variable or prompt"),
            );
        }
        if tags.user_operator.is_none() {
            let _ = tags.user_operator.insert(
                get_user_id()
                    .or_else(|| {
                        if self.non_interactive {
                            None
                        } else {
                            Input::with_theme(&*theme)
                                .with_prompt(
                                    "Enter your name/username (who is running this pipeline)",
                                )
                                .default(std::env::var("USER").unwrap_or_else(|_| "unknown".into()))
                                .interact_text()
                                .inspect_err(|e| {
                                    panic!("Error while prompting for user operator: {e}")
                                })
                                .ok()
                        }
                    })
                    .expect("Failed to get user operator from environment variable or prompt"),
            );
        }

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
