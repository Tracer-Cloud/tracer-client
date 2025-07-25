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

    let is_batch = env::var("AWS_BATCH_JOB_ID").is_ok();

    if let Some(metadata) = get_aws_instance_metadata().await {
        let instance_type = &metadata.instance_type;
        annotate_ec2_metadata(&metadata);
        if is_batch {
            return format!("AWS Batch - {instance_type}");
        }
        return if running_in_docker {
            format!("AWS EC2 (Docker) - {instance_type}")
        } else {
            format!("AWS EC2 - {instance_type}")
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
            println!("error getting metadata: {err}");
            None
        }
    }
}
fn annotate_ec2_metadata(metadata: &InstanceMetadata) {
    crate::Sentry::add_tag("aws_instance_id", &metadata.instance_id);
    crate::Sentry::add_tag("aws_region", metadata.region);
    crate::Sentry::add_tag("aws_account_id", &metadata.account_id);
    crate::Sentry::add_tag("aws_ami_id", &metadata.ami_id);
    crate::Sentry::add_tag("aws_instance_type", &metadata.instance_type);
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
