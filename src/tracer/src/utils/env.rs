use std::env;
use std::fs;
use std::path::Path;

// Environment variables that control init parameters
pub const TRACE_ID_ENV_VAR: &str = "TRACER_TRACE_ID";

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

pub(crate) async fn detect_environment_type() -> String {
    let running_in_docker = is_docker();

    if is_codespaces() {
        return "GitHub Codespaces".into();
    }

    if get_env_var(GITHUB_ACTIONS_ENV_VAR)
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        return "GitHub Actions".into();
    }

    if has_env_var(AWS_BATCH_JOB_ID_ENV_VAR) {
        return "AWS Batch".into();
    }

    if detect_ec2_environment().await.is_some() {
        return if running_in_docker {
            "AWS EC2 (Docker)".into()
        } else {
            "AWS EC2".into()
        };
    }

    if is_docker() {
        return "Docker".into();
    }

    "Local".into()
}

fn is_codespaces() -> bool {
    has_env_var(CODESPACES_ENV_VAR)
        || has_env_var(CODESPACE_NAME_ENV_VAR)
        || get_env_var(HOSTNAME_ENV_VAR)
            .map(|v| v.contains("codespaces-"))
            .unwrap_or(false)
}

async fn detect_ec2_environment() -> Option<String> {
    // Try DMI UUID
    if let Ok(uuid) = fs::read_to_string("/sys/devices/virtual/dmi/id/product_uuid") {
        if uuid.to_lowercase().starts_with("ec2") {
            return Some("AWS EC2".into());
        }
    }
    // Fallback to metadata service
    let url = "http://169.254.169.254/latest/meta-data/instance-id";
    if let Ok(resp) = reqwest::get(url).await {
        if resp.status() == 200 {
            return Some("AWS EC2".into());
        }
    }

    None
}
