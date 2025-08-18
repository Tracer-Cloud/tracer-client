use crate::utils::env::{self, USER_ID_ENV_VAR};
use crate::warning_message;
use anyhow::Result;
use colored::Colorize;

use super::sentry_context::{create_reporter_with_context, UserIdSentryReporter};
use super::shell_file_parser::read_user_id_from_file;

/// Main user ID extraction function with comprehensive fallback strategies
/// Tries multiple sources in priority order with full Sentry error reporting
pub fn extract_user_id(current_user_id: Option<String>) -> Result<String> {
    let mut sentry_reporter = create_reporter_with_context("user_id_extraction", "extract_user_id");

    // High-level resolution steps in priority order
    try_provided_user_id(current_user_id, &mut sentry_reporter)
        .or_else(|| try_environment_variable(&mut sentry_reporter))
        .or_else(|| try_shell_configuration_files(&mut sentry_reporter))
        .or_else(|| try_system_username_fallback(&mut sentry_reporter))
        .unwrap_or_else(|| {
            sentry_reporter.report_all_strategies_failed(&[
                "provided_user_id".to_string(),
                "environment_variable".to_string(),
                "shell_config_files".to_string(),
                "system_username".to_string(),
            ]);
            Err(anyhow::anyhow!("Failed to extract user ID from any available source"))
        })
}

/// Step 1: Try using the provided user_id parameter
fn try_provided_user_id(
    current_user_id: Option<String>,
    sentry_reporter: &mut UserIdSentryReporter
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
fn try_shell_configuration_files(sentry_reporter: &mut UserIdSentryReporter) -> Option<Result<String>> {
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
            // Error already reported in read_user_id_from_shell_configs
        }
    }
    None
}

/// Step 4: Try system username as fallback (with warning)
fn try_system_username_fallback(sentry_reporter: &mut UserIdSentryReporter) -> Option<Result<String>> {
    if let Some(username) = env::get_env_var("USER") {
        if !username.trim().is_empty() {
            warning_message!(
                "Failed to get user ID from environment variable or shell config files. \
                Defaulting to the system username '{}', which may not be your Tracer user ID! \
                Please set the TRACER_USER_ID environment variable or run the installer.",
                username
            );

            sentry_reporter.report_system_username_fallback(&username);
            return Some(Ok(username));
        }
    }
    None
}

/// Reads user ID from shell configuration files (.zshrc, .bashrc, etc.)
/// Returns Ok(Some(user_id)) if found, Ok(None) if not found, Err if IO error
fn read_user_id_from_shell_configs(sentry_reporter: &mut UserIdSentryReporter) -> Result<Option<String>> {
    const SHELL_CONFIG_FILES: &[&str] = &[
        ".zshrc",
        ".bashrc", 
        ".zprofile",
        ".bash_profile",
        ".profile",
    ];

    let home = match dirs_next::home_dir() {
        Some(home) => home,
        None => {
            let error = anyhow::anyhow!("Could not find home directory");
            sentry_reporter.report_home_directory_error(&error);
            return Err(error);
        }
    };

    let export_pattern = format!("export {}=", USER_ID_ENV_VAR);
    
    for config_file in SHELL_CONFIG_FILES {
        let config_path = home.join(config_file);
        
        if !config_path.exists() {
            continue;
        }

        match read_user_id_from_file(&config_path, &export_pattern) {
            Ok(Some(user_id)) => {
                return Ok(Some(user_id));
            }
            Ok(None) => {
                // Continue to next file
            }
            Err(e) => {
                sentry_reporter.report_shell_config_read_error(
                    &config_path.to_string_lossy(),
                    &e
                );
                // Continue to next file instead of failing
            }
        }
    }

    Ok(None)
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
        env::set_var(USER_ID_ENV_VAR, "env_test_user");
        let result = extract_user_id(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "env_test_user");
        env::remove_var(USER_ID_ENV_VAR);
    }


}
