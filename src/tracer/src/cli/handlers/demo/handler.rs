use crate::cli::handlers::demo::arguments::TracerCliDemoArgs;
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
    // Handle help-advanced flag
    if args.help_advanced {
        print_advanced_help();
        return Ok(());
    }

    // Handle list command
    if args.is_list_command() {
        let repo = TracerPipelinesRepo::new()?;
        let pipelines = repo.list_pipelines();
        println!("Available demo pipelines:");
        for pipeline in pipelines {
            println!("  {}", pipeline.name());
        }
        return Ok(());
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

fn print_advanced_help() {
    println!(
        r#"Advanced options:
      --config FILE       Path to config file
      --env-var K=V       Extra env vars for collector (repeatable)
      --watch-dir DIR     Directory to watch for logs (default: cwd)
      --environment-type  Execution environment (e.g., GitHub Actions, AWS EC2)
      --force             Terminate existing daemon before starting new one
      --force-procfs      Use /proc polling instead of eBPF
      --log-level         Log level [trace|debug|info|warn|error] (default: info)

Metadata tags:
      --pipeline-type     Type of pipeline (e.g., preprocessing, RNA-seq)
      --department        Department (default: Research)
      --team              Team (default: Oncology)
      --organization-id   Organization ID
  -u, --user-id           User ID (default: auto-resolved)
"#
    );
}
