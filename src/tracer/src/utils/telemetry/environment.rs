use crate::utils::env::*;

// Normalized environment name constants for ClickHouse storage
const ENV_AWS_EC2: &str = "aws-ec2";
const ENV_AWS_BATCH: &str = "aws-batch";
const ENV_GITHUB_CODESPACES: &str = "github-codespaces";
const ENV_GITHUB_ACTIONS: &str = "github-actions";
const ENV_DOCKER: &str = "docker";
const ENV_LOCAL: &str = "local";

/// Error categories for telemetry reporting
#[derive(Debug, Clone, Copy)]
pub enum ErrorCategory {
    NetworkFailure,
    SerializationFailure,
    Non2xxResponse,
    JsonParseFailure,
    DatabaseError,
    FileSystemError,
    ConfigurationError,
    AuthenticationError,
    ValidationError,
    TimeoutError,
    UnknownError,
}

impl ErrorCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCategory::NetworkFailure => "network_failure",
            ErrorCategory::SerializationFailure => "serialization_failure",
            ErrorCategory::Non2xxResponse => "non_2xx_response",
            ErrorCategory::JsonParseFailure => "json_parse_failure",
            ErrorCategory::DatabaseError => "database_error",
            ErrorCategory::FileSystemError => "filesystem_error",
            ErrorCategory::ConfigurationError => "configuration_error",
            ErrorCategory::AuthenticationError => "authentication_error",
            ErrorCategory::ValidationError => "validation_error",
            ErrorCategory::TimeoutError => "timeout_error",
            ErrorCategory::UnknownError => "unknown_error",
        }
    }
}

/// Detect the current execution environment
pub fn detect_environment() -> String {
    // Check for Docker
    #[cfg(target_os = "linux")]
    let is_docker = {
        use std::fs;
        use std::path::Path;
        Path::new("/.dockerenv").exists()
            || fs::read_to_string("/proc/1/cgroup")
                .map(|content| content.contains("docker") || content.contains("containerd"))
                .unwrap_or(false)
    };

    #[cfg(not(target_os = "linux"))]
    let is_docker = false; // Docker detection not implemented for non-Linux platforms

    // Check for Codespaces
    if has_env_var(CODESPACES_ENV_VAR)
        || has_env_var(CODESPACE_NAME_ENV_VAR)
        || get_env_var(HOSTNAME_ENV_VAR)
            .map(|v| v.contains("codespaces-"))
            .unwrap_or(false)
    {
        return ENV_GITHUB_CODESPACES.to_string();
    }

    // Check for GitHub Actions
    if has_env_var(GITHUB_ACTIONS_ENV_VAR) {
        return ENV_GITHUB_ACTIONS.to_string();
    }

    // Check for AWS Batch
    if has_env_var(AWS_BATCH_JOB_ID_ENV_VAR) {
        return ENV_AWS_BATCH.to_string();
    }

    // Check for Docker (if detected earlier)
    if is_docker {
        return ENV_DOCKER.to_string();
    }

    // Check for AWS EC2 (Linux only)
    #[cfg(target_os = "linux")]
    if let Ok(uuid) = std::fs::read_to_string("/sys/devices/virtual/dmi/id/product_uuid") {
        if uuid.to_lowercase().starts_with("ec2") {
            return ENV_AWS_EC2.to_string();
        }
    }

    // Default to local development
    ENV_LOCAL.to_string()
}
