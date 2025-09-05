// src/tracer/src/cli/handlers/init/arguments/resolver.rs
use super::super::user_prompts::{print_help, UserPrompts};
use super::user_id_resolver::resolve_user_id;
use super::{FinalizedInitArgs, PromptMode, TracerCliInitArgs};
use crate::constants::JWT_TOKEN_FILE_PATH;
use crate::utils::env;
use crate::utils::jwt_utils::claims::Claims;
use crate::utils::jwt_utils::jwt;
use crate::utils::jwt_utils::jwt::is_jwt_valid;
use std::collections::HashMap;

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

        let user_id = self.resolve_user_id(&prompt_mode).await;

        // Resolve environment type first so it can be used in pipeline name generation
        self.resolve_environment_type().await;

        let pipeline_name = self.resolve_pipeline_name(&prompt_mode, &user_id);
        let run_name = self.resolve_run_name();

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

    async fn resolve_user_id(&mut self, prompt_mode: &PromptMode) -> String {
        // let user_id = resolve_user_id(self.args.tags.user_id.clone(), prompt_mode);
        let token_claims = self.get_token_claims().await;

        if token_claims.is_some() {
            let token_claims = token_claims.unwrap();
            let user_id = token_claims.sub;

            self.args.tags.user_id = Some(user_id.to_string());

            println!("User ID detected: {}", user_id);

            return user_id;
        }

        "".to_string()
    }

    fn resolve_pipeline_name(&self, prompt_mode: &PromptMode, user_id: &str) -> String {
        match (self.args.pipeline_name.clone(), prompt_mode) {
            (Some(name), PromptMode::Required) => {
                // Only prompt for confirmation in Required mode
                UserPrompts::prompt_for_pipeline_name(&name)
                    .unwrap_or_else(|| {
                        eprintln!("Warning: Using provided pipeline name '{}' (could not prompt for confirmation)", name);
                        name
                    })
            }
            (Some(name), _) => {
                // If the pipeline name is "test" or "demo", expand it to include user_id
                if name == "test" {
                    format!("test-{}", user_id)
                } else if name == "demo" {
                    format!("demo-{}", user_id)
                } else if name.starts_with("demo-pipeline:") {
                    // For demo pipelines with specific pipeline ID: "demo-pipeline:{pipeline_id}" -> "{environment}-demo-{pipeline_id}-{user_id}"
                    let pipeline_id = name.strip_prefix("demo-pipeline:").unwrap();
                    let env_type = self
                        .args
                        .tags
                        .environment_type
                        .as_ref()
                        .map(|env| {
                            env.to_lowercase()
                                .replace(" ", "-")
                                .replace("(", "")
                                .replace(")", "")
                        })
                        .unwrap_or_else(|| "local".to_string());
                    format!("{}-demo-{}-{}", env_type, pipeline_id, user_id)
                } else {
                    name
                }
            }
            (None, PromptMode::Minimal | PromptMode::Required) => {
                UserPrompts::prompt_for_pipeline_name(user_id).unwrap_or_else(|| {
                    // Generate pipeline name with environment prefix
                    let env_type = self
                        .args
                        .tags
                        .environment_type
                        .as_ref()
                        .map(|env| {
                            env.to_lowercase()
                                .replace(" ", "-")
                                .replace("(", "")
                                .replace(")", "")
                        })
                        .unwrap_or_else(|| "no-terminal".to_string());
                    let default_name = format!("{}-{}", env_type, user_id);
                    eprintln!(
                        "Warning: No terminal detected. Using generated pipeline name: '{}'",
                        default_name
                    );
                    eprintln!("To specify a custom pipeline name, use: --pipeline-name <name>");
                    default_name
                })
            }
            (None, PromptMode::None) => user_id.to_string(),
        }
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
        if self.args.tags.environment_type.is_none() {
            // Always detect environment for demo pipelines (they use demo-pipeline: prefix)
            let is_demo_pipeline = self
                .args
                .pipeline_name
                .as_ref()
                .map(|name| name.starts_with("demo-pipeline:"))
                .unwrap_or(false);

            if !self.args.no_daemonize || is_demo_pipeline {
                self.args.tags.environment_type = Some(env::detect_environment_type(1).await);
            }
        }
    }

    fn resolve_environment(&mut self, prompt_mode: &PromptMode) {
        let environment = match (&self.args.tags.environment, prompt_mode) {
            (Some(env), PromptMode::Required) => {
                Some(UserPrompts::prompt_for_environment_name(env))
            }
            (Some(name), _) => Some(name.clone()),
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
        let pipeline_type = match (&self.args.tags.pipeline_type, prompt_mode) {
            (Some(env), PromptMode::Required) => UserPrompts::prompt_for_pipeline_type(env),
            (Some(env), _) => env.clone(),
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

    async fn get_token_claims(&self) -> Option<Claims> {
        // read the token.txt file
        let token = if let Ok(token) = std::fs::read_to_string(JWT_TOKEN_FILE_PATH.to_string()) {
            Some(token)
        } else {
            None
        };

        if token.is_none() {
            return None;
        }

        let is_token_valid_with_claims = is_jwt_valid(token.unwrap().as_str()).await;

        if is_token_valid_with_claims.0 {
            Some(is_token_valid_with_claims.1.unwrap())
        } else {
            None
        }
    }
}
