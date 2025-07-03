#![cfg(target_os = "linux")]
use crate::cli::handlers::info;
use crate::cli::helper::{handle_port_conflict, wait};
use crate::daemon::client::DaemonClient;
use crate::process_identification::constants::{
    DEFAULT_DAEMON_PORT, LOG_FILE, PID_FILE, STDERR_FILE, STDOUT_FILE, WORKING_DIR,
};
use crate::process_identification::types::cli::params::FinalizedInitArgs;
use crate::utils::analytics;
use crate::utils::analytics::types::AnalyticsEventType;
use anyhow::Context;
use daemonize::{Daemonize, Outcome};
use std::fs::File;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter};

pub(super) fn linux_no_daemonize(
    args: &FinalizedInitArgs,
    api_client: DaemonClient,
) -> anyhow::Result<bool> {
    match start_daemon() {
        Outcome::Parent(Ok(_)) => {
            println!("\nDaemon started successfully.");

            tokio::runtime::Runtime::new()?.block_on(async {
                analytics::spawn_event(
                    args.user_id.clone(),
                    AnalyticsEventType::DaemonStartAttempted,
                    None,
                );
                wait(&api_client).await?;

                info(&api_client, false).await
            })?;

            return Ok(true);
        }
        Outcome::Parent(Err(e)) => {
            println!("Failed to start daemon. Maybe the daemon is already running? If it's not, run `tracer cleanup` to clean up the previous daemon files.");
            println!("{:}", e);
            // Try to clean up port if there's an error
            let _ =
                tokio::runtime::Runtime::new()?.block_on(handle_port_conflict(DEFAULT_DAEMON_PORT));
            Ok(true)
        }
        Outcome::Child(Err(e)) => {
            anyhow::bail!(e);
        }
        Outcome::Child(Ok(_)) => {
            setup_logging()?;
            Ok(false)
        }
    }
}

fn start_daemon() -> Outcome<()> {
    let daemon = Daemonize::new()
        .pid_file(PID_FILE)
        .working_directory(WORKING_DIR)
        .stdout(File::create(STDOUT_FILE).expect("Failed to create stdout file"))
        .stderr(File::create(STDERR_FILE).expect("Failed to create stderr file"))
        .umask(0o002)
        .privileged_action(|| {
            // Ensure the PID file is removed if the process exits
            let _ = std::fs::remove_file(PID_FILE);
        });

    daemon.execute()
}
fn setup_logging() -> anyhow::Result<()> {
    // Set up the filter
    let filter = EnvFilter::from("debug"); // Capture all levels from debug up

    // Create a file appender that writes to daemon.log
    let file_appender = RollingFileAppender::new(Rotation::NEVER, WORKING_DIR, "daemon.log");

    // Create a custom format for the logs
    let file_layer = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(true)
        .with_level(true)
        .with_timer(SystemTime)
        .with_writer(file_appender);

    // Set up the subscriber with our custom layer
    let subscriber = tracing_subscriber::registry().with(filter).with(file_layer);

    // Set the subscriber as the default
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    // Log initialization message
    tracing::info!("Logging system initialized. Writing to {}", LOG_FILE);

    Ok(())
}
