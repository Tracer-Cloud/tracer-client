use crate::utils::Sentry;
use serde_json::json;
use std::collections::HashMap;

/// Sentry error reporting for user ID resolution failures
pub struct UserIdSentryReporter {
    context: HashMap<String, serde_json::Value>,
}

impl UserIdSentryReporter {
    pub fn new() -> Self {
        Self {
            context: HashMap::new(),
        }
    }

    /// Add context information for the resolution attempt
    pub fn add_context(&mut self, key: &str, value: serde_json::Value) {
        self.context.insert(key.to_string(), value);
    }

    /// Report error when user_id is not available from environment variable
    pub fn report_env_var_missing(&self, env_var_name: &str) {
        let error_context = json!({
            "error_type": "user_id_env_var_missing",
            "env_var_name": env_var_name,
            "severity": "warning",
            "resolution_context": self.context,
            "message": format!("Environment variable {} is not set or empty", env_var_name),
            "impact": "Falling back to shell config files or system username",
            "recommendation": format!("Set {} environment variable or run tracer-installer", env_var_name)
        });

        Sentry::add_context("user_id_resolution_error", error_context.clone());

        // Also capture as a Sentry error for monitoring
        Sentry::capture_message(
            &format!(
                "User ID environment variable {} not available",
                env_var_name
            ),
            sentry::Level::Warning,
        );
    }

    /// Report error when user_id is not available from shell config files
    pub fn report_shell_config_missing(&self, attempted_files: &[String]) {
        let error_context = json!({
            "error_type": "user_id_shell_config_missing",
            "attempted_files": attempted_files,
            "severity": "warning",
            "resolution_context": self.context,
            "message": "No TRACER_USER_ID found in any shell configuration files",
            "impact": "Falling back to system username",
            "recommendation": "Run tracer-installer to properly configure shell files"
        });

        Sentry::add_context("user_id_resolution_error", error_context.clone());

        // Also capture as a Sentry error for monitoring
        Sentry::capture_message(
            &format!(
                "User ID not found in shell config files: {:?}",
                attempted_files
            ),
            sentry::Level::Warning,
        );
    }

    /// Report error when shell config file cannot be read
    pub fn report_shell_config_read_error(&self, file_path: &str, error: &anyhow::Error) {
        let error_context = json!({
            "error_type": "user_id_shell_config_read_error",
            "file_path": file_path,
            "error_message": error.to_string(),
            "severity": "error",
            "resolution_context": self.context,
            "message": format!("Failed to read shell config file: {}", file_path),
            "impact": "Cannot read user_id from this config file",
            "recommendation": "Check file permissions and existence"
        });

        Sentry::add_context("user_id_resolution_error", error_context.clone());

        // Capture as a Sentry error with the actual error
        Sentry::capture_message(&error.to_string(), sentry::Level::Error);
    }

    /// Report critical error when all resolution strategies fail
    pub fn report_all_strategies_failed(&self, attempted_strategies: &[String]) {
        let error_context = json!({
            "error_type": "user_id_all_strategies_failed",
            "attempted_strategies": attempted_strategies,
            "severity": "critical",
            "resolution_context": self.context,
            "message": "All user ID resolution strategies failed",
            "impact": "Cannot determine user identity for tracer operations",
            "recommendation": "Set TRACER_USER_ID environment variable or run tracer-installer"
        });

        Sentry::add_context("user_id_resolution_error", error_context.clone());

        // Capture as a critical Sentry error
        Sentry::capture_message(
            &format!(
                "Critical: All user ID resolution strategies failed: {:?}",
                attempted_strategies
            ),
            sentry::Level::Error,
        );
    }

    /// Report successful resolution with strategy used
    /// Note: This method intentionally does NOT send Sentry alerts for successful user ID resolution
    /// as success cases should not generate monitoring alerts
    pub fn report_success(&self, _strategy: &str, _user_id: &str) {
        // Intentionally empty - we don't want to send Sentry alerts for successful user ID resolution
        // Success is the expected behavior and should not generate monitoring noise
    }

    /// Report when home directory cannot be found
    pub fn report_home_directory_error(&self, error: &anyhow::Error) {
        let error_context = json!({
            "error_type": "user_id_home_directory_error",
            "error_message": error.to_string(),
            "severity": "error",
            "resolution_context": self.context,
            "message": "Cannot find home directory for shell config file reading",
            "impact": "Cannot read shell configuration files",
            "recommendation": "Check HOME environment variable and user permissions"
        });

        Sentry::add_context("user_id_resolution_error", error_context.clone());

        // Capture as a Sentry error
        Sentry::capture_message(&error.to_string(), sentry::Level::Error);
    }
}

impl Default for UserIdSentryReporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a reporter with common context
pub fn create_reporter_with_context(operation: &str, source: &str) -> UserIdSentryReporter {
    let mut reporter = UserIdSentryReporter::new();
    reporter.add_context("operation", json!(operation));
    reporter.add_context("source", json!(source));
    reporter.add_context("timestamp", json!(chrono::Utc::now().to_rfc3339()));
    reporter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reporter_creation() {
        let reporter = UserIdSentryReporter::new();
        assert!(reporter.context.is_empty());
    }

    #[test]
    fn test_add_context() {
        let mut reporter = UserIdSentryReporter::new();
        reporter.add_context("test_key", json!("test_value"));
        assert_eq!(reporter.context.get("test_key"), Some(&json!("test_value")));
    }

    #[test]
    fn test_create_reporter_with_context() {
        let reporter = create_reporter_with_context("test_operation", "test_source");
        assert_eq!(
            reporter.context.get("operation"),
            Some(&json!("test_operation"))
        );
        assert_eq!(reporter.context.get("source"), Some(&json!("test_source")));
        assert!(reporter.context.contains_key("timestamp"));
    }
}
