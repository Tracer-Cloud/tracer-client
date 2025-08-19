use crate::cli::handlers::demo::arguments::TracerCliDemoArgs;
use crate::cli::handlers::demo::command_handlers::DemoCommandHandlers;
use crate::cli::handlers::demo::daemon_execution::{
    run_demo_with_existing_daemon, run_demo_with_new_daemon,
};

use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::utils::system_info::check_sudo_with_procfs_option;

use anyhow::Result;

/// TODO: fastquorum segfault on ARM mac; Rosetta/x86 pixi option may be needed.
pub async fn demo(args: TracerCliDemoArgs, config: Config, api_client: DaemonClient) -> Result<()> {
    // Handle help-advanced flag
    if args.help_advanced {
        return DemoCommandHandlers::handle_help_advanced();
    }

    // Handle list command
    if args.is_list_command() {
        return DemoCommandHandlers::handle_list_command();
    }

    // Handle pipeline execution (including default case)
    let (init_args, selected_demo_pipeline) = args.resolve_demo_arguments()?;
    check_sudo_with_procfs_option("demo", init_args.force_procfs);
    let daemon_was_already_running = DaemonServer::is_running();

    if daemon_was_already_running {
        run_demo_with_existing_daemon(&api_client, selected_demo_pipeline).await
    } else {
        run_demo_with_new_daemon(init_args, config, &api_client, selected_demo_pipeline).await
    }
}
