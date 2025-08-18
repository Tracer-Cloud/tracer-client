use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use colored::Colorize;
use crate::daemon::server::DaemonServer;
use crate::utils::system_info::check_sudo;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::info_message;

use super::daemon_spawn::spawn_daemon_process;
use super::existing_daemon::handle_existing_daemon;
use super::sentry_context::setup_sentry_context;
use super::setup_daemon_logging::setup_daemon_logging;

/// Performs initial setup and validation before starting daemon
async fn perform_init_setup(args: &TracerCliInitArgs, api_client: &DaemonClient) -> anyhow::Result<()> {
    if !args.force_procfs && cfg!(target_os = "linux") {
        // Check if running with sudo
        check_sudo("init");
    }

    // Create work dir for logging and daemonizing files
    TRACER_WORK_DIR
        .init()
        .expect("Error while creating necessary files");

    // Check for existing daemon and handle appropriately
    if DaemonServer::is_running() {
        handle_existing_daemon(args, api_client).await?;
    }

    Ok(())
}

/// Initialize the tracer daemon with the given pipeline prefix
pub async fn init(
    mut args: TracerCliInitArgs,
    config: Config,
    api_client: &DaemonClient,
) -> anyhow::Result<()> {
    const DEFAULT_PIPELINE_PREFIX: &str = "pipeline";
    // Perform initial setup and validation
    perform_init_setup(&args, api_client).await?;

    info_message!("Starting daemon...");

    // Force non-interactive mode when running as daemon process
    if args.no_daemonize {
        args.set_non_interactive();
    }

    let args = args.resolve_arguments(DEFAULT_PIPELINE_PREFIX).await;

    // Set up Sentry context for monitoring
    setup_sentry_context(&args)?;

    if !args.no_daemonize {
        // Spawn daemon process and wait for it to be ready
        return spawn_daemon_process(&args, api_client).await;
    }
    setup_daemon_logging(&args.log_level)?;
    DaemonServer::new().await.start(args, config).await
}


