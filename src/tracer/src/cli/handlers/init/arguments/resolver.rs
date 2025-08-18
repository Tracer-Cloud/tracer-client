// src/tracer/src/cli/handlers/init/arguments/resolver.rs
use crate::utils::env;
use std::collections::HashMap;

use super::super::user_prompts::{print_help, UserPrompts};
use super::arguments::{FinalizedInitArgs, PromptMode, TracerCliInitArgs};
use super::user_id_resolver::resolve_user_id;

/// Constants for argument resolution
pub const DEFAULT_PIPELINE_TYPE: &str = "Preprocessing";
pub const DEFAULT_ENVIRONMENT: &str = "local";

/// Handles argument resolution logic
pub struct ArgumentResolver {
    args: TracerCliInitArgs,
}

impl ArgumentResolver {
    pub fn new(args: TracerCliInitArgs) -> Self {
        Self { args }
    }

    pub async fn resolve(mut self) -> FinalizedInitArgs {
        let prompt_mode = self.args.interactive_prompts.clone();

        let user_id = self.resolve_user_id(&prompt_mode);
        let pipeline_name = self.resolve_pipeline_name(&prompt_mode, &user_id);
        let run_name = self.resolve_run_name();

        self.resolve_environment_type().await;
        self.resolve_environment(&prompt_mode);
        self.resolve_pipeline_type(&prompt_mode);
        let environment_variables = self.resolve_environment_variables();

        FinalizedInitArgs {
            pipeline_name,
            run_name,
            user_id,
            tags: self.args.tags,
            no_daemonize: self.args.no_daemonize,
            dev: self.args.dev,
            force_procfs: self.args.force_procfs,
            force: self.args.force,
            log_level: self.args.log_level,
            environment_variables,
            watch_dir: self.args.watch_dir,
        }
    }

    fn resolve_user_id(&mut self, prompt_mode: &PromptMode) -> String {
        let user_id = resolve_user_id(self.args.tags.user_id.clone(), prompt_mode);
        self.args.tags.user_id = Some(user_id.clone());
        user_id
    }

    fn resolve_pipeline_name(&self, prompt_mode: &PromptMode, user_id: &str) -> String {
        match (self.args.pipeline_name.clone(), prompt_mode) {
            (Some(name), PromptMode::Required) => {
                // Only prompt for confirmation in Required mode
                Some(UserPrompts::prompt_for_pipeline_name(&name))
            }
            (Some(name), _) => Some(name),
            (None, PromptMode::Minimal | PromptMode::Required) => {
                Some(UserPrompts::prompt_for_pipeline_name(user_id))
            }
            (None, PromptMode::None) => Some(user_id.to_string()),
        }
        .or_else(print_help)
        .expect("Failed to get pipeline name from command line, environment variable, or prompt")
    }

    fn resolve_run_name(&self) -> Option<String> {
        // Ignore empty run names
        self.args
            .run_name
            .clone()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
    }

    async fn resolve_environment_type(&mut self) {
        // this call can take a while - if this is the daemon process being spawned, defer it until
        // we create the client, otherwise use a short timeout so the init call doesn't take too long
        if self.args.tags.environment_type.is_none() && !self.args.no_daemonize {
            self.args.tags.environment_type = Some(env::detect_environment_type(1).await);
        }
    }

    fn resolve_environment(&mut self, prompt_mode: &PromptMode) {
        let environment = match (self.args.tags.environment.clone(), prompt_mode) {
            (Some(env), PromptMode::Required) => {
                Some(UserPrompts::prompt_for_environment_name(&env))
            }
            (Some(name), _) => Some(name),
            (None, PromptMode::Required) if self.args.tags.environment_type.is_some() => {
                Some(UserPrompts::prompt_for_environment_name(
                    self.args.tags.environment_type.as_ref().unwrap(),
                ))
            }
            (None, PromptMode::Required) => Some(UserPrompts::prompt_for_environment_name(
                DEFAULT_ENVIRONMENT,
            )),
            (None, _) if self.args.tags.environment_type.is_some() => {
                self.args.tags.environment_type.clone()
            }
            (None, _) => Some(DEFAULT_ENVIRONMENT.to_string()),
        }
        .or_else(print_help)
        .expect("Failed to get environment from command line, environment variable, or prompt");

        self.args.tags.environment = Some(environment);
    }

    fn resolve_pipeline_type(&mut self, prompt_mode: &PromptMode) {
        let pipeline_type = match (self.args.tags.pipeline_type.clone(), prompt_mode) {
            (Some(env), PromptMode::Required) => UserPrompts::prompt_for_pipeline_type(&env),
            (Some(env), _) => env,
            (None, PromptMode::Required) => {
                UserPrompts::prompt_for_pipeline_type(DEFAULT_PIPELINE_TYPE)
            }
            (None, _) => DEFAULT_PIPELINE_TYPE.to_string(),
        };

        self.args.tags.pipeline_type = Some(pipeline_type);
    }

    fn resolve_environment_variables(&self) -> HashMap<String, String> {
        let mut environment_variables = HashMap::new();
        for env_var in &self.args.env_var {
            if let Some((key, value)) = env_var.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() {
                    environment_variables.insert(key.to_string(), value.to_string());
                }
            }
        }
        environment_variables
    }


}
