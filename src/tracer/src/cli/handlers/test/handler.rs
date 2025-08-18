use crate::cli::handlers::test::arguments::TracerCliTestArgs;
use crate::cli::handlers::test::daemon_execution::{
    run_test_with_existing_daemon, run_test_with_new_daemon,
};

use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::utils::system_info::check_sudo_with_procfs_option;

use anyhow::Result;

/// TODO: fastquorum segfault on ARM mac; Rosetta/x86 pixi option may be needed.
pub async fn test(args: TracerCliTestArgs, config: Config, api_client: DaemonClient) -> Result<()> {
    // this is the entry function for the test command
    check_sudo_with_procfs_option("test", args.init_args.force_procfs);

    // Resolve the pipeline early so we can pass it to both functions
    let (init_args, selected_test_pipeline) = args.resolve_test_arguments()?;
    let daemon_was_already_running = DaemonServer::is_running();

    if daemon_was_already_running {
        run_test_with_existing_daemon(&api_client, selected_test_pipeline).await
    } else {
        run_test_with_new_daemon(init_args, config, &api_client, selected_test_pipeline).await
    }
}
