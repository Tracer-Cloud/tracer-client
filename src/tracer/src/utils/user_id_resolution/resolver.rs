use crate::info_message;
use crate::utils::env::{self, USER_ID_ENV_VAR};
use anyhow::Result;
use colored::Colorize;

use super::error_reporter::report_user_id_resolution_failure;
use super::sentry_context::{create_reporter_with_context, UserIdSentryReporter};
use super::shell_config_reader::read_user_id_from_shell_configs;

/// Main user ID extraction function with comprehensive fallback strategies
/// Tries multiple sources in priority order with full Sentry error reporting
pub fn extract_user_id(current_user_id: Option<String>) -> Result<String> {
    let mut sentry_reporter = create_reporter_with_context("user_id_extraction", "extract_user_id");

    info_message!("Resolving user ID...");

    // High-level resolution steps in priority order
    let result = try_provided_user_id(current_user_id, &mut sentry_reporter)
        .or_else(|| try_environment_variable(&mut sentry_reporter))
        .or_else(|| try_shell_configuration_files(&mut sentry_reporter))
        .unwrap_or_else(|| report_user_id_resolution_failure(&mut sentry_reporter));

    // Show final result
    match &result {
        Ok(user_id) => info_message!("User ID resolved: {}", user_id),
        Err(_) => info_message!("User ID resolution failed"),
    }

    result
}

/// Step 1: Try using the provided user_id parameter
fn try_provided_user_id(
    current_user_id: Option<String>,
    sentry_reporter: &mut UserIdSentryReporter,
) -> Option<Result<String>> {
    if let Some(user_id) = current_user_id {
        if !user_id.trim().is_empty() {
            sentry_reporter.report_success("provided_user_id", &user_id);
            return Some(Ok(user_id));
        }
    }
    None
}

/// Step 2: Try reading from TRACER_USER_ID environment variable
fn try_environment_variable(sentry_reporter: &mut UserIdSentryReporter) -> Option<Result<String>> {
    if let Some(user_id) = env::get_env_var(USER_ID_ENV_VAR) {
        if !user_id.trim().is_empty() {
            sentry_reporter.report_success("environment_variable", &user_id);
            return Some(Ok(user_id));
        }
    } else {
        sentry_reporter.report_env_var_missing(USER_ID_ENV_VAR);
    }
    None
}

/// Step 3: Try reading from shell configuration files (.zshrc, .bashrc, etc.)
fn try_shell_configuration_files(
    sentry_reporter: &mut UserIdSentryReporter,
) -> Option<Result<String>> {
    match read_user_id_from_shell_configs(sentry_reporter) {
        Ok(Some(user_id)) => {
            if !user_id.trim().is_empty() {
                sentry_reporter.report_success("shell_config_files", &user_id);
                return Some(Ok(user_id));
            }
        }
        Ok(None) => {
            let attempted_files = vec![
                ".zshrc".to_string(),
                ".bashrc".to_string(),
                ".zprofile".to_string(),
                ".bash_profile".to_string(),
                ".profile".to_string(),
            ];
            sentry_reporter.report_shell_config_missing(&attempted_files);
        }
        Err(_) => {
            // Error already logged by shell config reader
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_extract_user_id_with_provided_id() {
        let result = extract_user_id(Some("test_user".to_string()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_user");
    }

    #[test]
    fn test_extract_user_id_with_env_var() {
        // Remove any existing env var first
        env::remove_var(USER_ID_ENV_VAR);

        // Set our test value
        env::set_var(USER_ID_ENV_VAR, "env_test_user");
        let result = extract_user_id(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "env_test_user");

        // Clean up
        env::remove_var(USER_ID_ENV_VAR);
    }

    // Note: The failure test is challenging to write because it depends on the actual
    // shell configuration files on the system. In a real test environment, we would
    // need to mock the file system or use dependency injection to control the shell
    // config reading behavior. For now, we'll rely on integration tests for this scenario.
}
