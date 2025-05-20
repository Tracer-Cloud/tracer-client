use dialoguer::Input;
use whoami;

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
    // pub fn from_partial(cli_args: TracerCliInitArgs) -> Self {
    //     let default = InteractiveInitArgs::default();

    //     Self {
    //         pipeline_name: cli_args.pipeline_name.clone().or(default.pipeline_name),
    //         run_id: cli_args.run_id.clone(),
    //         tags: PipelineTags {
    //             environment: cli_args
    //                 .tags
    //                 .environment
    //                 .clone()
    //                 .or(default.tags.environment),

    //             pipeline_type: cli_args
    //                 .tags
    //                 .pipeline_type
    //                 .clone()
    //                 .or(default.tags.pipeline_type),

    //             user_operator: cli_args
    //                 .tags
    //                 .user_operator
    //                 .clone()
    //                 .or(default.tags.user_operator),

    //             department: cli_args.tags.department.clone(),
    //             team: cli_args.tags.team.clone(),
    //             organization_id: cli_args.tags.organization_id.clone(),
    //             others: cli_args.tags.others.clone(),
    //         },
    //         no_daemonize: cli_args.no_daemonize,
    //         is_dev: cli_args.is_dev,
    //     }
    // }

    pub fn prompt_missing(mut self) -> Self {
        if self.pipeline_name.is_none() {
            self.pipeline_name = Some(
                Input::new()
                    .with_prompt("Enter pipeline name")
                    .default("demo_pipeline".into())
                    .interact_text()
                    .unwrap(),
            );
        }

        if self.tags.environment.is_none() {
            self.tags.environment = Some(
                Input::new()
                    .with_prompt("Environment")
                    .default("local".into())
                    .interact_text()
                    .unwrap(),
            );
        }

        if self.tags.pipeline_type.is_none() {
            self.tags.pipeline_type = Some(
                Input::new()
                    .with_prompt("Pipeline Type")
                    .default("generic".into())
                    .interact_text()
                    .unwrap(),
            );
        }

        if self.tags.user_operator.is_none() {
            self.tags.user_operator = Some(whoami::username());
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

    // println!("Starting tracer with:");
    // println!(
    //     "Pipeline: {}",
    //     interactive_args.pipeline_name.clone().unwrap()
    // );
    // println!(
    //     "Environment: {}",
    //     interactive_args.tags.environment.clone().unwrap()
    // );
    // println!(
    //     "Pipeline Type: {}",
    //     interactive_args.tags.pipeline_type.clone().unwrap()
    // );
    // println!(
    //     "User: {}",
    //     interactive_args.tags.user_operator.clone().unwrap()
    // );
}
