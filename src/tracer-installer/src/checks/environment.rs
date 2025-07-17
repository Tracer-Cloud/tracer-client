use std::env;
use std::fs;
use std::path::Path;

use super::InstallCheck;

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
pub async fn detect_environment_type() -> String {
    let running_in_docker = is_docker();

    if is_codespaces() {
        return "GitHub Codespaces".into();
    }

    if env::var("GITHUB_ACTIONS").is_ok_and(|v| v == "true") {
        return "GitHub Actions".into();
    }

    if env::var("AWS_BATCH_JOB_ID").is_ok() {
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
    env::var("CODESPACES").is_ok()
        || env::var("CODESPACE_NAME").is_ok()
        || env::var("HOSTNAME").is_ok_and(|v| v.contains("codespaces-"))
}

async fn detect_ec2_environment() -> Option<String> {
    // Try DMI UUID
    if let Ok(uuid) = fs::read_to_string("/sys/devices/virtual/dmi/id/product_uuid") {
        if uuid.to_lowercase().starts_with("ec2") {
            return Some("AWS EC2".into());
        }
    }

    // Fallback to metadata service with 200ms timeout
    let url = "http://169.254.169.254/latest/meta-data/instance-id";
    let client = reqwest::Client::new();
    if let Ok(resp) = client
        .get(url)
        .timeout(std::time::Duration::from_millis(200))
        .send()
        .await
    {
        if resp.status() == 200 {
            return Some("AWS EC2".into());
        }
    }

    None
}

pub struct EnvironmentCheck {
    detected: String,
}

impl EnvironmentCheck {
    pub async fn new() -> Self {
        let detected = detect_environment_type().await;
        crate::Sentry::add_tag("detected_env", &detected);
        Self { detected }
    }
}

#[async_trait::async_trait]
impl InstallCheck for EnvironmentCheck {
    async fn check(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Environment Type"
    }

    fn error_message(&self) -> String {
        "Unknown".into()
    }

    fn success_message(&self) -> String {
        self.detected.to_string()
    }
}
