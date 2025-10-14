use crate::constants::environment::*;
use crate::constants::{SANDBOX_URL_DEV, SANDBOX_URL_PROD};
use reqwest::Client;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

// Environment variables that control init parameters
pub const TRACE_ID_ENV_VAR: &str = "TRACER_TRACE_ID";
pub const USER_ID_ENV_VAR: &str = "TRACER_USER_ID";

// Environment variables that control environment detection
pub const GITHUB_ACTIONS_ENV_VAR: &str = "GITHUB_ACTIONS";
pub const AWS_BATCH_JOB_ID_ENV_VAR: &str = "AWS_BATCH_JOB_ID";
pub const CODESPACES_ENV_VAR: &str = "CODESPACES";
pub const CODESPACE_NAME_ENV_VAR: &str = "CODESPACE_NAME";
pub const HOSTNAME_ENV_VAR: &str = "HOSTNAME";

pub fn get_env_var(var: &str) -> Option<String> {
    env::var(var).ok()
}

pub fn has_env_var(var: &str) -> bool {
    get_env_var(var).is_some()
}

fn is_docker() -> bool {
    // 1. Check for /.dockerenv
    if Path::new("/.dockerenv").exists() {
        return true;
    }

    // 2. Inspect /proc/1/cgroup for docker or containerd references
    if let Ok(content) = fs::read_to_string("/proc/1/cgroup") {
        if content.contains("docker") || content.contains("containerd") {
            return true;
        }
    }

    false
}

/// Try to detect the environment type as one of Docker, GitHub Codespaces,
/// GitHub Actions, AWS Batch, AWS EC2, or Local. When checking whether the
/// environment is EC2, this function may query the metadata server -
/// `timeout_secs` is used to set the timeout for that query.
pub(crate) async fn detect_environment_type(timeout_secs: u64) -> String {
    let running_in_docker = is_docker();

    if is_codespaces() {
        return ENV_GITHUB_CODESPACES.into();
    }

    if get_env_var(GITHUB_ACTIONS_ENV_VAR)
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        return ENV_GITHUB_ACTIONS.into();
    }

    if has_env_var(AWS_BATCH_JOB_ID_ENV_VAR) {
        return ENV_AWS_BATCH.into();
    }

    if detect_ec2_environment(timeout_secs).await.is_some() {
        return ENV_AWS_EC2.into();
    }

    if running_in_docker {
        return ENV_DOCKER.into();
    }

    ENV_LOCAL.into()
}

fn is_codespaces() -> bool {
    has_env_var(CODESPACES_ENV_VAR)
        || has_env_var(CODESPACE_NAME_ENV_VAR)
        || get_env_var(HOSTNAME_ENV_VAR)
            .map(|v| v.contains("codespaces-"))
            .unwrap_or(false)
}

async fn detect_ec2_environment(timeout_secs: u64) -> Option<String> {
    // Try DMI UUID
    if let Ok(uuid) = fs::read_to_string("/sys/devices/virtual/dmi/id/product_uuid") {
        if uuid.to_lowercase().starts_with("ec2") {
            return Some("AWS EC2".into());
        }
    }
    // Fallback to metadata service
    let url = "http://169.254.169.254/latest/meta-data/instance-id";
    if let Ok(client) = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
    {
        if let Ok(resp) = client.get(url).send().await {
            if resp.status() == 200 {
                return Some("AWS EC2".into());
            }
        }
    }

    None
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
