use anyhow::Result;
use std::path::PathBuf;

use super::sentry_context::UserIdSentryReporter;
use super::shell_file_parser::read_user_id_from_file;

/// Reads user ID from shell configuration files (.zshrc, .bashrc, etc.)
/// Returns Ok(Some(user_id)) if found, Ok(None) if not found, Err if IO error
pub fn read_user_id_from_shell_configs(
    sentry_reporter: &mut UserIdSentryReporter,
) -> Result<Option<String>> {
    read_user_id_from_shell_configs_with_home_dir(sentry_reporter, get_home_directory)
}

/// Testable version that accepts a home directory provider function
pub fn read_user_id_from_shell_configs_with_home_dir<F>(
    sentry_reporter: &mut UserIdSentryReporter,
    home_dir_provider: F,
) -> Result<Option<String>>
where
    F: Fn() -> Result<PathBuf>,
{
    let home = match home_dir_provider() {
        Ok(home) => home,
        Err(error) => {
            sentry_reporter.report_home_directory_error(&error);
            return Err(error);
        }
    };

    let config_files = [
        ".zshrc",
        ".bashrc",
        ".zprofile",
        ".bash_profile",
        ".profile",
    ];
    let export_pattern = "export TRACER_USER_ID=";

    for config_file in &config_files {
        let config_path = home.join(config_file);

        if !config_path.exists() {
            continue;
        }

        match read_user_id_from_file(&config_path, export_pattern) {
            Ok(Some(user_id)) => {
                return Ok(Some(user_id));
            }
            Ok(None) => {
                // Continue to next file
            }
            Err(e) => {
                sentry_reporter.report_shell_config_read_error(&config_path.to_string_lossy(), &e);
                // Continue to next file instead of failing
            }
        }
    }

    Ok(None)
}

/// Default home directory provider
fn get_home_directory() -> Result<PathBuf> {
    dirs_next::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::user_id_resolution::sentry_context::create_reporter_with_context;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    // Mock home directory provider for testing
    fn create_test_home_with_files(files: Vec<(&str, &str)>) -> Result<TempDir> {
        let temp_dir = TempDir::new()?;

        for (filename, content) in files {
            let file_path = temp_dir.path().join(filename);
            let mut file = fs::File::create(file_path)?;
            writeln!(file, "{}", content)?;
        }

        Ok(temp_dir)
    }

    #[test]
    fn test_read_user_id_from_shell_configs_success_zshrc() -> Result<()> {
        let temp_dir = create_test_home_with_files(vec![(
            ".zshrc",
            r#"export TRACER_USER_ID="test_user_zsh""#,
        )])?;

        let mut reporter = create_reporter_with_context("test", "test");
        let home_path = temp_dir.path().to_path_buf();

        let result =
            read_user_id_from_shell_configs_with_home_dir(&mut reporter, || Ok(home_path.clone()))?;

        assert_eq!(result, Some("test_user_zsh".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_shell_configs_success_bashrc() -> Result<()> {
        let temp_dir =
            create_test_home_with_files(vec![(".bashrc", "export TRACER_USER_ID=test_user_bash")])?;

        let mut reporter = create_reporter_with_context("test", "test");
        let home_path = temp_dir.path().to_path_buf();

        let result =
            read_user_id_from_shell_configs_with_home_dir(&mut reporter, || Ok(home_path.clone()))?;

        assert_eq!(result, Some("test_user_bash".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_shell_configs_priority_order() -> Result<()> {
        // .zshrc should be checked first and win
        let temp_dir = create_test_home_with_files(vec![
            (".zshrc", r#"export TRACER_USER_ID="first_user""#),
            (".bashrc", r#"export TRACER_USER_ID="second_user""#),
        ])?;

        let mut reporter = create_reporter_with_context("test", "test");
        let home_path = temp_dir.path().to_path_buf();

        let result =
            read_user_id_from_shell_configs_with_home_dir(&mut reporter, || Ok(home_path.clone()))?;

        assert_eq!(result, Some("first_user".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_shell_configs_not_found() -> Result<()> {
        let temp_dir = create_test_home_with_files(vec![
            (".zshrc", "# No TRACER_USER_ID here"),
            (".bashrc", "export OTHER_VAR=value"),
        ])?;

        let mut reporter = create_reporter_with_context("test", "test");
        let home_path = temp_dir.path().to_path_buf();

        let result =
            read_user_id_from_shell_configs_with_home_dir(&mut reporter, || Ok(home_path.clone()))?;

        assert_eq!(result, None);
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_shell_configs_no_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut reporter = create_reporter_with_context("test", "test");
        let home_path = temp_dir.path().to_path_buf();

        let result =
            read_user_id_from_shell_configs_with_home_dir(&mut reporter, || Ok(home_path.clone()))?;

        assert_eq!(result, None);
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_shell_configs_home_directory_error() {
        let mut reporter = create_reporter_with_context("test", "test");

        let result = read_user_id_from_shell_configs_with_home_dir(&mut reporter, || {
            Err(anyhow::anyhow!("Home directory not found"))
        });

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Home directory not found"));
    }

    #[test]
    fn test_read_user_id_from_shell_configs_multiple_formats() -> Result<()> {
        let temp_dir = create_test_home_with_files(vec![(
            ".zprofile",
            r#"export TRACER_USER_ID='single_quotes'"#,
        )])?;

        let mut reporter = create_reporter_with_context("test", "test");
        let home_path = temp_dir.path().to_path_buf();

        let result =
            read_user_id_from_shell_configs_with_home_dir(&mut reporter, || Ok(home_path.clone()))?;

        assert_eq!(result, Some("single_quotes".to_string()));
        Ok(())
    }

    #[test]
    fn test_read_user_id_from_shell_configs_empty_value() -> Result<()> {
        let temp_dir =
            create_test_home_with_files(vec![(".bash_profile", r#"export TRACER_USER_ID="""#)])?;

        let mut reporter = create_reporter_with_context("test", "test");
        let home_path = temp_dir.path().to_path_buf();

        let result =
            read_user_id_from_shell_configs_with_home_dir(&mut reporter, || Ok(home_path.clone()))?;

        assert_eq!(result, None);
        Ok(())
    }
}
