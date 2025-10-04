/// tracer/src/cli/handlers/init/handler.rs
use super::arguments::TracerCliInitArgs;
use super::setup::{
    handle_existing_daemon, setup_daemon_logging, setup_sentry_context, spawn_daemon_process,
};
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::info_message;
use crate::utils::env::is_development_environment;
use crate::utils::system_info::check_sudo_with_procfs_option;
use crate::utils::workdir::TRACER_WORK_DIR;
use colored::Colorize;

/// Initialize the tracer daemon with the given pipeline prefix
pub async fn init(
    mut args: TracerCliInitArgs,
    config: Config,
    api_client: &DaemonClient,
) -> anyhow::Result<()> {
    // Perform initial setup and validation
    init_setup_validation(&args, api_client).await?;

    info_message!("Starting daemon...");

    // Force non-interactive mode when running as a daemon process
    if args.no_daemonize {
        args.set_non_interactive();
    }

    // Set dev mode to true if running in the dev environment
    args.dev = is_development_environment();

    let args = args.resolve_arguments().await;

    // Set up Sentry context for monitoring
    setup_sentry_context(&args)?;

    if args.no_daemonize {
        setup_daemon_logging(&args.log_level)?;
        DaemonServer::new().await.start(args, config).await
    } else {
        // Spawn the daemon process and wait for it to be ready
        spawn_daemon_process(&args, api_client).await
    }
}

/// Performs initial setup and validation before starting the daemon
async fn init_setup_validation(
    args: &TracerCliInitArgs,
    api_client: &DaemonClient,
) -> anyhow::Result<()> {
    // Check if running with sudo (Linux only, unless force_procfs is enabled)
    check_sudo_with_procfs_option("init", args.force_procfs);

    // Create a work dir for logging and daemonizing files
    TRACER_WORK_DIR
        .init()
        .expect("Error while creating necessary files");

    // Check for existing daemon and handle appropriately
    if DaemonServer::is_running() {
        handle_existing_daemon(args, api_client).await?;
    }

    Ok(())
}
