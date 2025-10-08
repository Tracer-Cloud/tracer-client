use crate::sentry::Sentry;
use ec2_instance_metadata::InstanceMetadata;
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
    if is_codespaces() {
        return "GitHub Codespaces".into();
    }

    let running_in_docker = is_docker();

    if let Some(_metadata) = get_aws_instance_metadata().await {
        return if running_in_docker {
            "AWS EC2 (Docker)".to_string()
        } else {
            "AWS EC2".to_string()
        };
    }

    if running_in_docker {
        return "Docker".into();
    }

    "Local".into()
}

fn is_codespaces() -> bool {
    env::var("CODESPACES").is_ok()
        || env::var("CODESPACE_NAME").is_ok()
        || env::var("HOSTNAME").is_ok_and(|v| v.contains("codespaces-"))
}

pub async fn get_aws_instance_metadata() -> Option<InstanceMetadata> {
    let client = ec2_instance_metadata::InstanceMetadataClient::new();
    match client.get() {
        Ok(metadata) => Some(metadata),
        Err(err) => {
            let msg = format!("error getting metadata: {err}");
            Sentry::capture_message(&msg, sentry::Level::Error);
            println!("{}", msg);
            None
        }
    }
}

pub struct EnvironmentCheck {
    detected: String,
}

impl EnvironmentCheck {
    pub async fn new() -> Self {
        let detected = detect_environment_type().await;
        Sentry::add_tag("detected_env", &detected);
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
