use anyhow::Result;
use colored::Colorize;

use super::sentry_context::UserIdSentryReporter;

/// Provide comprehensive error reporting when all user ID resolution strategies fail
pub fn report_user_id_resolution_failure(
    sentry_reporter: &mut UserIdSentryReporter,
) -> Result<String> {
    println!("\n{}", "❌ USER ID RESOLUTION FAILED".red().bold());
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".red()
    );

    println!(
        "\n{}",
        "Unable to determine your Tracer user ID from any of the following sources:".red()
    );
    println!("  • Command line parameter (--user-id)");
    println!("  • Environment variable (TRACER_USER_ID)");
    println!("  • Shell configuration files (.zshrc, .bashrc, .zprofile, .bash_profile, .profile)");

    println!("\n{}", "🔧 HOW TO FIX THIS:".yellow().bold());

    println!(
        "\n{}",
        "Option 1: Get your user ID from Tracer Cloud Sandbox"
            .cyan()
            .bold()
    );
    println!(
        "  Visit: {}",
        "https://sandbox.tracer.cloud/onboarding/github-codespaces"
            .blue()
            .underline()
    );
    println!("  Copy your user ID and use one of the methods below.");

    println!(
        "\n{}",
        "Option 2: Pass as command line parameter".cyan().bold()
    );
    println!("  tracer init --user-id \"your-user-id-here\"");

    println!("\n{}", "Option 3: Set environment variable".cyan().bold());
    println!("  export TRACER_USER_ID=\"your-user-id-here\"");

    println!("\n{}", "Option 4: Run the tracer installer".cyan().bold());
    println!("  curl -sSL https://install.tracer.cloud | bash");
    println!("  (This will automatically configure your shell)");

    println!(
        "\n{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".red()
    );

    sentry_reporter.report_all_strategies_failed(&[
        "provided_user_id".to_string(),
        "environment_variable".to_string(),
        "shell_config_files".to_string(),
    ]);

    Err(anyhow::anyhow!(
        "User ID could not be resolved from any available source"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::user_id_resolution::sentry_context::create_reporter_with_context;

    #[test]
    fn test_report_user_id_resolution_failure() {
        let mut reporter = create_reporter_with_context("test", "test");
        let result = report_user_id_resolution_failure(&mut reporter);

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("User ID could not be resolved"));
    }
}
