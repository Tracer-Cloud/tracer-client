use std::env;
use std::fs;
use std::path::Path;

use ec2_instance_metadata::InstanceMetadata;

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

    if let Some(metadata) = get_aws_instance_metadata().await {
        crate::Sentry::add_tag("aws_instance_id", &metadata.instance_id);
        crate::Sentry::add_tag("aws_region", metadata.region);
        crate::Sentry::add_tag("aws_account_id", &metadata.account_id);
        crate::Sentry::add_tag("aws_ami_id", &metadata.ami_id);
        crate::Sentry::add_tag("aws_instance_type", &metadata.instance_type);

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

pub async fn get_aws_instance_metadata() -> Option<InstanceMetadata> {
    let client = ec2_instance_metadata::InstanceMetadataClient::new();
    match client.get() {
        Ok(metadata) => Some(metadata),
        Err(err) => {
            println!("error getting metadata: {err}");
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
