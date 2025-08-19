use crate::cli::handlers::demo::arguments::{DemoCommand, TracerCliDemoArgs};
use crate::cli::handlers::demo::daemon_execution::{
    run_demo_with_existing_daemon, run_demo_with_new_daemon,
};
use crate::cli::handlers::demo::git_repo_pipelines::TracerPipelinesRepo;

use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::utils::system_info::check_sudo_with_procfs_option;

use anyhow::Result;

/// TODO: fastquorum segfault on ARM mac; Rosetta/x86 pixi option may be needed.
pub async fn demo(args: TracerCliDemoArgs, config: Config, api_client: DaemonClient) -> Result<()> {
    // this is the entry function for the demo command
    match args.command {
        DemoCommand::List => {
            // Handle list command
            let repo = TracerPipelinesRepo::new()?;
            let pipelines = repo.list_pipelines();
            println!("Available demo pipelines:");
            for pipeline in pipelines {
                println!("  {}", pipeline.name());
            }
            Ok(())
        }
        DemoCommand::Fastquorum { ref init_args }
        | DemoCommand::Wdl { ref init_args }
        | DemoCommand::Run { ref init_args, .. } => {
            // Handle pipeline execution commands
            check_sudo_with_procfs_option("demo", init_args.force_procfs);

            // Resolve the pipeline early so we can pass it to both functions
            let (init_args, selected_demo_pipeline) = args.resolve_demo_arguments()?;
            let daemon_was_already_running = DaemonServer::is_running();

            if daemon_was_already_running {
                run_demo_with_existing_daemon(&api_client, selected_demo_pipeline).await
            } else {
                run_demo_with_new_daemon(init_args, config, &api_client, selected_demo_pipeline).await
            }
        }
    }
}
