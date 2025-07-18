use super::commands::{Cli, Command};
use super::handlers;
use crate::config::Config;
use crate::daemon::server::DaemonServer;
use crate::utils::{Sentry, Version};
use anyhow::Result;
use clap::Parser;

/// Process the command line.
/// Note: this has to be sync due to daemonizing
pub fn process_command() -> Result<()> {
    // setting env var to prevent fork safety issues on macOS
    // TODO: can we annotate this with #[cfg(target_os = "macos")]?
    std::env::set_var("OBJC_DISABLE_INITIALIZE_FORK_SAFETY", "YES");

    // NOTE: this panics if there is a parsing error
    let cli = Cli::parse();

    // Use the --config flag, if provided, when loading the configuration
    let config = Config::default();

    let _guard = Sentry::setup();
    Sentry::add_context("Config", config.to_safe_json());

    match cli.command {
        Command::Cleanup => {
            DaemonServer::cleanup();
            println!("Daemon files cleanup completed.");
            Ok(())
        }
        Command::Version => {
            println!("{}", Version::current());
            Ok(())
        }
        Command::Update => handlers::update(),
        Command::Uninstall => handlers::uninstall(),
        command => {
            tokio::runtime::Runtime::new()?.block_on(super::process_daemon_command(command, config))
        }
    }
}
