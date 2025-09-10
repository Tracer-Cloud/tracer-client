use super::super::arguments::FinalizedInitArgs;
use crate::cli::handlers::{info, otel_start_with_auto_install};
use crate::cli::helper::wait;
use colored::Colorize;

use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::utils::analytics::types::AnalyticsEventType;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::utils::{analytics, spawn};
use crate::{error_message, info_message, success_message, warning_message};

/// Spawns a new daemon process and waits for it to be ready
pub async fn spawn_daemon_process(
    args: &FinalizedInitArgs,
    api_client: &DaemonClient,
) -> anyhow::Result<()> {
    DaemonServer::cleanup();

    info_message!("Spawning child process...");

    let spawn_args = build_spawn_args(args);
    let spawn_args_str: Vec<&str> = spawn_args.iter().map(|s| s.as_str()).collect();
    let child_id = spawn::spawn_child(&spawn_args_str)?;

    std::fs::write(&TRACER_WORK_DIR.pid_file, child_id.to_string())?;
    success_message!("Daemon started successfully.");

    // Wait for the daemon to be ready, then show info
    analytics::spawn_event(
        args.user_id.clone(),
        AnalyticsEventType::DaemonStartAttempted,
        None,
    );

    if !wait(api_client).await {
        error_message!("Daemon is not responding, please check logs");
        return Ok(());
    }

    success_message!("Daemon is ready and responding");

    // Always try to start the OTEL collector during init
    if let Err(e) = otel_start_with_auto_install(args.watch_dir.clone(), true).await {
        error_message!("Failed to start OpenTelemetry collector: {}", e);
        warning_message!("Continuing without OpenTelemetry collector. You can start it later with 'tracer otel start'");
    }

    info(api_client, false).await;

    Ok(())
}

/// Builds the command line arguments for spawning the daemon process
fn build_spawn_args(args: &FinalizedInitArgs) -> Vec<String> {
    let mut spawn_args = vec![
        "init".to_string(),
        "--no-daemonize".to_string(),
        "--pipeline-name".to_string(),
        args.pipeline_name.clone(),
        "--environment".to_string(),
        args.tags.environment.as_deref().unwrap_or("").to_string(),
        "--pipeline-type".to_string(),
        args.tags.pipeline_type.as_deref().unwrap_or("").to_string(),
        "--token".to_string(),
        args.tags.user_id.as_deref().unwrap().to_string(),
        "--log-level".to_string(),
        args.log_level.clone(),
    ];

    if args.dev {
        spawn_args.push("--dev".to_string());
    }
    if args.force_procfs {
        spawn_args.push("--force-procfs".to_string());
    }

    // Add environment variables for OTEL if provided
    for (key, value) in &args.environment_variables {
        spawn_args.push("--env-var".to_string());
        spawn_args.push(format!("{}={}", key, value));
    }

    spawn_args
}
