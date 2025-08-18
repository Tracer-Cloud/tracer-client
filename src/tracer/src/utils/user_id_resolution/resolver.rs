use crate::utils::env::{self, USER_ID_ENV_VAR};
use crate::warning_message;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use super::sentry_context::{create_reporter_with_context, UserIdSentryReporter};

/// Comprehensive user ID resolution with multiple fallback strategies
/// Instrumented with Sentry for monitoring and debugging
pub fn resolve_user_id_robust(current_user_id: Option<String>) -> Result<String> {
    let mut resolution_attempts: Vec<String> = Vec::new();
    let mut reporter = create_reporter_with_context("user_id_resolution", "resolve_user_id_robust");

    // Strategy 1: Use provided user_id
    if let Some(user_id) = current_user_id {
        if !user_id.trim().is_empty() {
            reporter.report_success("provided_user_id", &user_id);
            return Ok(user_id);
        }
        resolution_attempts.push("provided_user_id: empty".to_string());
    } else {
        resolution_attempts.push("provided_user_id: none".to_string());
    }

    // Strategy 2: Environment variable TRACER_USER_ID
    if let Some(user_id) = env::get_env_var(USER_ID_ENV_VAR) {
        if !user_id.trim().is_empty() {
            reporter.report_success("env_tracer_user_id", &user_id);
            return Ok(user_id);
        }
        resolution_attempts.push("env_tracer_user_id: empty".to_string());
    } else {
        reporter.report_env_var_missing(USER_ID_ENV_VAR);
        resolution_attempts.push("env_tracer_user_id: not_set".to_string());
    }

    // Strategy 3: Read from shell configuration files
    match read_user_id_from_shell_configs(&mut reporter) {
        Ok(Some(user_id)) => {
            if !user_id.trim().is_empty() {
                reporter.report_success("shell_config_files", &user_id);
                return Ok(user_id);
            }
            resolution_attempts.push("shell_config_files: empty".to_string());
        }
        Ok(None) => {
            let attempted_files = vec![
                ".zshrc".to_string(),
                ".bashrc".to_string(),
                ".zprofile".to_string(),
                ".bash_profile".to_string(),
                ".profile".to_string(),
            ];
            reporter.report_shell_config_missing(&attempted_files);
            resolution_attempts.push("shell_config_files: not_found".to_string());
        }
        Err(e) => {
            resolution_attempts.push(format!("shell_config_files: error({})", e));
        }
    }

    // Strategy 4: System username fallback
    if let Some(username) = env::get_env_var("USER") {
        if !username.trim().is_empty() {
            warning_message!(
                "Failed to get user ID from environment variable or shell config files. \
                Defaulting to the system username '{}', which may not be your Tracer user ID! \
                Please set the TRACER_USER_ID environment variable or run the installer.",
                username
            );

            reporter.report_system_username_fallback(&username);
            return Ok(username);
        }
        resolution_attempts.push("system_username: empty".to_string());
    } else {
        resolution_attempts.push("system_username: not_set".to_string());
    }

    // All strategies failed
    reporter.report_all_strategies_failed(&resolution_attempts);
    anyhow::bail!("Failed to resolve user ID from any source: {:?}", resolution_attempts)
}

/// Reads user ID from shell configuration files (.zshrc, .bashrc, etc.)
/// Returns Ok(Some(user_id)) if found, Ok(None) if not found, Err if IO error
fn read_user_id_from_shell_configs(reporter: &mut UserIdSentryReporter) -> Result<Option<String>> {
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
            reporter.report_home_directory_error(&error);
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
                reporter.report_shell_config_read_error(
                    &config_path.to_string_lossy(),
                    &e
                );
                // Continue to next file instead of failing
            }
        }
    }

    Ok(None)
}

/// Reads user ID from a specific shell configuration file
fn read_user_id_from_file(file_path: &PathBuf, export_pattern: &str) -> Result<Option<String>> {
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {:?}", file_path))?;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(export_pattern) {
            // Extract value from: export TRACER_USER_ID="value" or export TRACER_USER_ID=value
            let value_part = &line[export_pattern.len()..];
            let user_id = if value_part.starts_with('"') && value_part.ends_with('"') {
                // Remove quotes: "value" -> value
                value_part[1..value_part.len()-1].to_string()
            } else if value_part.starts_with('\'') && value_part.ends_with('\'') {
                // Remove single quotes: 'value' -> value
                value_part[1..value_part.len()-1].to_string()
            } else {
                // No quotes: value
                value_part.to_string()
            };
            
            if !user_id.trim().is_empty() {
                return Ok(Some(user_id));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_resolve_user_id_with_provided_id() {
        let result = resolve_user_id_robust(Some("test_user".to_string()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_user");
    }

    #[test]
    fn test_resolve_user_id_with_env_var() {
        env::set_var(USER_ID_ENV_VAR, "env_test_user");
        let result = resolve_user_id_robust(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "env_test_user");
        env::remove_var(USER_ID_ENV_VAR);
    }

    #[test]
    fn test_read_user_id_from_file_with_quotes() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "# Some comment")?;
        writeln!(temp_file, r#"export TRACER_USER_ID="quoted_user""#)?;
        writeln!(temp_file, "# Another comment")?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, Some("quoted_user".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_file_without_quotes() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "export TRACER_USER_ID=unquoted_user")?;
        
        let result = read_user_id_from_file(&temp_file.path().to_path_buf(), "export TRACER_USER_ID=")?;
        assert_eq!(result, Some("unquoted_user".to_string()));
        Ok(())
    }
}
