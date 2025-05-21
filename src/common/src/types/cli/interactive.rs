use console::Emoji;
use console::Style;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;

use crate::types::pipeline_tags::PipelineTags;

use super::params::{FinalizedInitArgs, TracerCliInitArgs};

#[derive(Debug, Clone)]
pub struct InteractiveInitArgs {
    pub pipeline_name: Option<String>,
    pub run_id: Option<String>,
    pub tags: PipelineTags,
    pub no_daemonize: bool,
    pub is_dev: Option<bool>,
}

impl Default for InteractiveInitArgs {
    fn default() -> Self {
        Self {
            pipeline_name: Some("demo_pipeline".into()),
            run_id: None,
            tags: PipelineTags::default(),
            no_daemonize: false,
            is_dev: Some(false),
        }
    }
}

impl InteractiveInitArgs {
    pub fn from_partial(cli_args: TracerCliInitArgs) -> Self {
        Self {
            pipeline_name: cli_args.pipeline_name,
            run_id: cli_args.run_id,
            tags: PipelineTags {
                environment: cli_args.tags.environment,
                pipeline_type: cli_args.tags.pipeline_type,
                user_operator: cli_args.tags.user_operator,
                department: cli_args.tags.department,
                team: cli_args.tags.team,
                organization_id: cli_args.tags.organization_id,
                others: cli_args.tags.others,
            },
            no_daemonize: cli_args.no_daemonize,
            is_dev: cli_args.is_dev,
        }
    }

    pub fn prompt_missing(mut self) -> Self {
        let arrow = Emoji("👉 ", "> ").to_string();
        let theme = ColorfulTheme {
            prompt_prefix: Style::new().green().apply_to(arrow),
            prompt_suffix: Style::new().dim().apply_to(":".to_string()),
            success_prefix: Style::new().green().apply_to("✔".to_string()),
            success_suffix: Style::new().dim().apply_to("".to_string()),
            values_style: Style::new().yellow(),
            active_item_style: Style::new().cyan().bold(),
            ..ColorfulTheme::default()
        };

        if self.pipeline_name.is_none() {
            self.pipeline_name = Some(
                Input::with_theme(&theme)
                    .with_prompt("Enter pipeline name")
                    .default("demo_pipeline".into())
                    .interact_text()
                    .unwrap(),
            );
        }

        if self.tags.environment.is_none() {
            self.tags.environment = Some(
                Input::with_theme(&theme)
                    .with_prompt("Environment")
                    .default("local".into())
                    .interact_text()
                    .unwrap(),
            );
        }

        if self.tags.pipeline_type.is_none() {
            self.tags.pipeline_type = Some(
                Input::with_theme(&theme)
                    .with_prompt("Pipeline Type")
                    .default("generic".into())
                    .interact_text()
                    .unwrap(),
            );
        }

        if self.tags.user_operator.is_none() {
            self.tags.user_operator = Some(
                Input::with_theme(&theme)
                    .with_prompt("User Operator")
                    .default(std::env::var("USER").unwrap_or_else(|_| "unknown".into()))
                    .interact_text()
                    .unwrap(),
            );
        }

        self
    }

    pub fn into_cli_args(self) -> FinalizedInitArgs {
        FinalizedInitArgs {
            pipeline_name: self.pipeline_name.expect("pipeline_name must be set"),
            run_id: self.run_id,
            tags: self.tags,
            no_daemonize: self.no_daemonize,
            is_dev: self.is_dev,
        }
    }
}

pub async fn run_init(cli_args: TracerCliInitArgs) -> FinalizedInitArgs {
    InteractiveInitArgs::from_partial(cli_args)
        .prompt_missing()
        .into_cli_args()
}
