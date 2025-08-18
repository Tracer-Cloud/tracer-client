mod setup_daemon_logging;
mod setup_sentry_context;
mod daemon_existing;
mod daemon_spawn;

// Re-export commonly used functions for convenience
pub use daemon_existing::handle_existing_daemon;
pub use daemon_spawn::spawn_daemon_process;
pub use setup_daemon_logging::setup_daemon_logging;
pub use setup_sentry_context::setup_sentry_context;
