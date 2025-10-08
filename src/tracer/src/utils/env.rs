use crate::constants::{SANDBOX_URL_DEV, SANDBOX_URL_PROD};
use std::env;

// Environment variables that control init parameters
pub const TRACE_ID_ENV_VAR: &str = "TRACER_TRACE_ID";
pub const USER_ID_ENV_VAR: &str = "TRACER_USER_ID";

// Environment variables that control environment detection
pub const GITHUB_ACTIONS_ENV_VAR: &str = "GITHUB_ACTIONS";
pub const AWS_BATCH_JOB_ID_ENV_VAR: &str = "AWS_BATCH_JOB_ID";
pub const CODESPACES_ENV_VAR: &str = "CODESPACES";
pub const CODESPACE_NAME_ENV_VAR: &str = "CODESPACE_NAME";
pub const HOSTNAME_ENV_VAR: &str = "HOSTNAME";
pub const TRACER_ENVIRONMENT_TYPE_ENV_VAR: &str = "TRACER_ENVIRONMENT_TYPE";

pub fn get_env_var(var: &str) -> Option<String> {
    env::var(var).ok()
}

pub fn has_env_var(var: &str) -> bool {
    get_env_var(var).is_some()
}

pub(crate) fn detect_environment_type() -> String {
    env::var(TRACER_ENVIRONMENT_TYPE_ENV_VAR).unwrap_or("Not detected".into())
}

/// Get the build channel (prod, dev, ...) from the environment variable.
/// Defaults to "prod" if the variable is not set.
pub fn get_build_channel() -> &'static str {
    let channel = env!("BUILD_CHANNEL");

    if channel.is_empty() {
        return "prod";
    }

    channel
}

pub fn is_development_environment() -> bool {
    get_build_channel() == "dev"
}

pub fn get_sandbox_url() -> &'static str {
    if is_development_environment() {
        SANDBOX_URL_DEV
    } else {
        SANDBOX_URL_PROD
    }
}
