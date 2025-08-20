use crate::utils::Sentry;
use serde_json::json;
use std::collections::HashMap;

/// Categories of errors for telemetry reporting
#[derive(Debug, Clone)]
pub enum ErrorCategory {
    NetworkFailure,
    Non2xxResponse,
    JsonParseFailure,
    SerializationFailure,
}

impl ErrorCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCategory::NetworkFailure => "network_failure",
            ErrorCategory::Non2xxResponse => "non_2xx_response",
            ErrorCategory::JsonParseFailure => "json_parse_failure",
            ErrorCategory::SerializationFailure => "serialization_failure",
        }
    }
}

/// Context builder for telemetry reporting
pub struct TelemetryContext {
    context: HashMap<String, serde_json::Value>,
}

impl TelemetryContext {
    pub fn new(component: &str, error_category: ErrorCategory) -> Self {
        let mut context = HashMap::new();

        // Base context
        context.insert("component".to_string(), json!(component));
        context.insert("error_type".to_string(), json!(error_category.as_str()));
        context.insert(
            "timestamp".to_string(),
            json!(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()),
        );

        // Add user context
        if let Some(user_id) = crate::utils::env::get_env_var(crate::utils::env::USER_ID_ENV_VAR) {
            if !user_id.trim().is_empty() {
                context.insert("user_id".to_string(), json!(user_id.trim()));
            }
        }

        // Add environment context
        context.insert("environment".to_string(), json!(detect_environment_sync()));

        // Add system context
        context.insert(
            "platform".to_string(),
            json!(crate::utils::system_info::get_platform_information()),
        );

        if let Some((major, minor)) = crate::utils::system_info::get_kernel_version() {
            context.insert(
                "kernel_version".to_string(),
                json!(format!("{}.{}", major, minor)),
            );
        }

        // Add process context
        if let Ok(cwd) = std::env::current_dir() {
            context.insert(
                "working_directory".to_string(),
                json!(cwd.to_string_lossy()),
            );
        }
        context.insert("process_id".to_string(), json!(std::process::id()));

        Self { context }
    }

    /// Add custom field to the context
    pub fn add_field<T: serde::Serialize>(mut self, key: &str, value: T) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.context.insert(key.to_string(), json_value);
        }
        self
    }

    /// Add endpoint information
    pub fn with_endpoint(self, endpoint: &str) -> Self {
        self.add_field("endpoint", endpoint)
    }

    /// Add HTTP status information
    pub fn with_http_status(self, status_code: u16, status_text: Option<&str>) -> Self {
        let mut updated = self.add_field("status_code", status_code);
        if let Some(text) = status_text {
            updated = updated.add_field("status_text", text);
        }
        updated
    }

    /// Add error information
    pub fn with_error<E>(self, error: &E) -> Self
    where
        E: std::fmt::Display + std::fmt::Debug,
    {
        self.add_field("error_message", error.to_string())
            .add_field("error_debug", format!("{:?}", error))
    }

    /// Add event count for batch operations
    pub fn with_event_count(self, count: usize) -> Self {
        self.add_field("event_count", count)
    }

    /// Add payload size information
    pub fn with_payload_size(self, size_bytes: usize) -> Self {
        self.add_field("payload_size_bytes", size_bytes)
    }

    /// Add response body (truncated for large responses)
    pub fn with_response_body(self, body: &str) -> Self {
        let truncated_body = if body.len() > 1000 {
            format!("{}... (truncated)", &body[..1000])
        } else {
            body.to_string()
        };
        self.add_field("response_body", truncated_body)
    }

    /// Convert to JSON value for Sentry
    pub fn to_json(self) -> serde_json::Value {
        serde_json::Value::Object(self.context.into_iter().collect())
    }

    /// Report to Sentry with the given key and message
    pub fn report_to_sentry(self, sentry_key: &str, message: &str, level: sentry::Level) {
        let json_context = self.to_json();
        Sentry::add_extra(sentry_key, json_context);
        Sentry::capture_message(message, level);
    }
}

/// Simplified synchronous environment detection for telemetry
fn detect_environment_sync() -> String {
    use crate::utils::env::*;
    use std::fs;
    use std::path::Path;

    // Check for Docker
    #[cfg(target_os = "linux")]
    let is_docker = Path::new("/.dockerenv").exists()
        || fs::read_to_string("/proc/1/cgroup")
            .map(|content| content.contains("docker") || content.contains("containerd"))
            .unwrap_or(false);

    #[cfg(not(target_os = "linux"))]
    let is_docker = false; // Docker detection not implemented for non-Linux platforms

    // Check for Codespaces
    if has_env_var(CODESPACES_ENV_VAR)
        || has_env_var(CODESPACE_NAME_ENV_VAR)
        || get_env_var(HOSTNAME_ENV_VAR)
            .map(|v| v.contains("codespaces-"))
            .unwrap_or(false)
    {
        return "GitHub Codespaces".to_string();
    }

    // Check for GitHub Actions
    if get_env_var(GITHUB_ACTIONS_ENV_VAR)
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        return "GitHub Actions".to_string();
    }

    // Check for AWS Batch
    if has_env_var(AWS_BATCH_JOB_ID_ENV_VAR) {
        return "AWS Batch".to_string();
    }

    // Check for EC2 (simplified - just check DMI UUID)
    if let Ok(uuid) = fs::read_to_string("/sys/devices/virtual/dmi/id/product_uuid") {
        if uuid.to_lowercase().starts_with("ec2") {
            return if is_docker {
                "AWS EC2 (Docker)".to_string()
            } else {
                "AWS EC2".to_string()
            };
        }
    }

    if is_docker {
        "Docker".to_string()
    } else {
        "Local".to_string()
    }
}

/// Convenience functions for common telemetry scenarios
pub mod presets {
    use super::*;

    /// Report a network failure
    pub fn report_network_failure<E>(component: &str, endpoint: &str, error: &E, message: &str)
    where
        E: std::fmt::Display + std::fmt::Debug,
    {
        TelemetryContext::new(component, ErrorCategory::NetworkFailure)
            .with_endpoint(endpoint)
            .with_error(error)
            .report_to_sentry(
                &format!("{}_network_error", component),
                message,
                sentry::Level::Error,
            );
    }

    /// Report a non-2XX HTTP response
    pub fn report_http_error(
        component: &str,
        endpoint: &str,
        status_code: u16,
        status_text: Option<&str>,
        response_body: Option<&str>,
        message: &str,
    ) {
        let mut context = TelemetryContext::new(component, ErrorCategory::Non2xxResponse)
            .with_endpoint(endpoint)
            .with_http_status(status_code, status_text);

        if let Some(body) = response_body {
            context = context.with_response_body(body);
        }

        context.report_to_sentry(
            &format!("{}_request_error", component),
            message,
            sentry::Level::Error,
        );
    }

    /// Report a JSON parsing failure
    pub fn report_json_parse_failure<E>(
        component: &str,
        endpoint: &str,
        status_code: u16,
        error: &E,
        message: &str,
    ) where
        E: std::fmt::Display + std::fmt::Debug,
    {
        TelemetryContext::new(component, ErrorCategory::JsonParseFailure)
            .with_endpoint(endpoint)
            .with_http_status(status_code, None)
            .with_error(error)
            .report_to_sentry(
                &format!("{}_json_parse_error", component),
                message,
                sentry::Level::Error,
            );
    }

    /// Report a serialization failure
    pub fn report_serialization_failure<E>(
        component: &str,
        error: &E,
        event_count: Option<usize>,
        message: &str,
    ) where
        E: std::fmt::Display + std::fmt::Debug,
    {
        let mut context =
            TelemetryContext::new(component, ErrorCategory::SerializationFailure).with_error(error);

        if let Some(count) = event_count {
            context = context.with_event_count(count);
        }

        context.report_to_sentry(
            &format!("{}_serialization_error", component),
            message,
            sentry::Level::Error,
        );
    }
}
