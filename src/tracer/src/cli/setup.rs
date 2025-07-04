use crate::process_identification::constants::{LOG_FILE, WORKING_DIR};
use anyhow::{Context, Result};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, time::SystemTime},
    prelude::*,
    EnvFilter,
};

pub fn setup_logging() -> Result<()> {
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
