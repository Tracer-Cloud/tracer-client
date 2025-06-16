use std::env;
use std::fs;
use std::process::Command;

use super::InstallCheck;

pub fn detect_environment_type() -> String {
    // 1. GitHub Codespaces
    if env::var("CODESPACES").is_ok() || env::var("CODESPACE_NAME").is_ok() {
        return "GitHub Codespaces".into();
    }

    // 2. GitHub Actions
    if env::var("GITHUB_ACTIONS").map_or(false, |v| v == "true") {
        return "GitHub Actions".into();
    }

    // 3. AWS Batch (via env var or metadata)
    if env::var("AWS_BATCH_JOB_ID").is_ok() {
        return "AWS Batch".into();
    }

    // 4. AWS EC2 (detect EC2 DMI product UUID)
    if let Ok(uuid) = fs::read_to_string("/sys/devices/virtual/dmi/id/product_uuid") {
        if uuid.starts_with("EC2") || uuid.to_lowercase().starts_with("ec2") {
            return "AWS EC2".into();
        }
    }

    // 5. Default
    Command::new("uname")
        .arg("-s")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "Unknown".into())
        .trim()
        .to_string()
}
pub struct EnvironmentCheck {
    detected: String,
}

impl EnvironmentCheck {
    pub fn new() -> Self {
        let detected = detect_environment_type();
        Self { detected }
    }
}

#[async_trait::async_trait]
impl InstallCheck for EnvironmentCheck {
    fn name(&self) -> &'static str {
        "Environment Type"
    }

    fn success_message(&self) -> String {
        format!("{}: {}", self.name(), self.detected)
    }

    fn error_message(&self) -> String {
        format!("{}: Unknown", self.name())
    }

    async fn check(&self) -> bool {
        true
    }
}
