use crate::cli::handlers::demo::git_repo_pipelines::TracerPipelinesRepo;
use anyhow::Result;

pub struct DemoCommandHandlers;

impl DemoCommandHandlers {
    /// Handle the --help-advanced flag
    pub fn handle_help_advanced() -> Result<()> {
        print_advanced_help();
        Ok(())
    }

    /// Handle the list command
    pub fn handle_list_command() -> Result<()> {
        let repo = TracerPipelinesRepo::new()?;
        let pipelines = repo.list_pipelines();
        println!("Available demo pipelines:");
        for pipeline in pipelines {
            println!("  {}", pipeline.name());
        }
        Ok(())
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
