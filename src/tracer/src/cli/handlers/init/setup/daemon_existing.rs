use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::handlers::terminate::{get_pid, terminate};
use crate::cli::handlers::INTERACTIVE_THEME;
use crate::daemon::client::DaemonClient;
use crate::{error_message, info_message, warning_message};
use colored::Colorize;
use dialoguer::Confirm;

/// Handles the case when a daemon is already running
pub async fn handle_existing_daemon(
    args: &TracerCliInitArgs,
    api_client: &DaemonClient,
) -> anyhow::Result<()> {
    let pid_info = get_pid()
        .map(|pid| format!(" (PID {})", pid))
        .unwrap_or_default();

    if args.force {
        warning_message!(
            "Daemon already running{}. Terminating due to --force flag...",
            pid_info
        );
        if !terminate(api_client).await {
            return Err(anyhow::anyhow!("Failed to terminate existing daemon"));
        }
        return Ok(());
    }

    // Check if we're in an interactive terminal (simplified check)
    if !args.no_daemonize {
        warning_message!("Daemon already running{}.", pid_info);
        let should_terminate = Confirm::with_theme(&*INTERACTIVE_THEME)
            .with_prompt("Terminate existing daemon and start new one?")
            .default(false)
            .interact()
            .unwrap_or(false);

        if should_terminate {
            info_message!("Terminating existing daemon...");
            if !terminate(api_client).await {
                return Err(anyhow::anyhow!("Failed to terminate existing daemon"));
            }
            return Ok(());
        } else {
            info_message!("Keeping existing daemon. Use 'tracer terminate' to stop it manually.");
            return Err(anyhow::anyhow!("Daemon already running"));
        }
    }

    // Non-interactive mode: refuse to start
    error_message!(
        "Daemon already running{}. Use 'tracer terminate' or 'tracer init --force' to restart.",
        pid_info
    );
    Err(anyhow::anyhow!("Daemon already running"))
}
