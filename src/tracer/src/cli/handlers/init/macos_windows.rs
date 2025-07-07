#![cfg(any(target_os = "macos", target_os = "windows"))]
use crate::cli::handlers::info;
use crate::cli::handlers::init::arguments::FinalizedInitArgs;
use crate::cli::helper::wait;
use crate::daemon::client::DaemonClient;
use crate::process_identification::constants::{PID_FILE, STDERR_FILE, STDOUT_FILE};
use crate::utils::analytics;
use crate::utils::analytics::types::AnalyticsEventType;
use std::fs::File;
use std::process::{Command, Stdio};

pub fn macos_windows_no_daemonize(
    args: FinalizedInitArgs,
    api_client: DaemonClient,
) -> anyhow::Result<()> {
    // Serialize the finalized args to pass to the spawned process
    let current_exe = std::env::current_exe()?;
    let is_dev_string = "false"; // for testing purposes //TODO remove

    println!("Spawning child process...");

    let child = Command::new(current_exe)
        .arg("init")
        .arg("--no-daemonize")
        .arg("--pipeline-name")
        .arg(&args.pipeline_name)
        .arg("--environment")
        .arg(args.tags.environment.as_deref().unwrap_or(""))
        .arg("--pipeline-type")
        .arg(args.tags.pipeline_type.as_deref().unwrap_or(""))
        .arg("--user-operator")
        .arg(args.tags.user_operator.as_deref().unwrap_or(""))
        .arg("--is-dev")
        .arg(is_dev_string)
        .stdin(Stdio::null())
        .stdout(Stdio::from(File::create(STDOUT_FILE)?))
        .stderr(Stdio::from(File::create(STDERR_FILE)?))
        .spawn()?;

    // Write PID file
    std::fs::write(PID_FILE, child.id().to_string())?;

    println!("\nDaemon started successfully.");

    // Wait a moment for daemon to start, then show info
    tokio::runtime::Runtime::new()?.block_on(async {
        analytics::spawn_event(
            args.user_id.clone(),
            AnalyticsEventType::DaemonStartAttempted,
            None,
        );
        wait(&api_client).await?;
        info(&api_client, false).await
    })?;

    Ok(())
}
