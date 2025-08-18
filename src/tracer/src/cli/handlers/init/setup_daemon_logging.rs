use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::Context;
use tracing_appender::rolling;
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter};

/// Sets up internal daemon logging to file
pub fn setup_daemon_logging(log_level: &String) -> anyhow::Result<()> {
    // Set up the filter
    // Capture all levels from log_level and up
    let filter = EnvFilter::from(log_level);

    // Create a file appender that writes to daemon.log
    let log_file = &TRACER_WORK_DIR.log_file;
    let file_appender = rolling::never(log_file.parent().unwrap(), log_file.file_name().unwrap());

    // Create a custom format for the logs without colors
    let file_layer = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(true)
        .with_level(true)
        .with_timer(SystemTime)
        .with_ansi(false) // This disables ANSI color codes
        .with_writer(file_appender);

    // Set up the subscriber with our custom layer
    let subscriber = tracing_subscriber::registry().with(filter).with(file_layer);

    // Set the subscriber as the default
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    // Log initialization message
    tracing::info!(
        "Logging system initialized. Writing to {:?}",
        TRACER_WORK_DIR.log_file
    );

    Ok(())
}
