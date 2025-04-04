use colored::Colorize;
use std::fmt::Write;
use std::process::Command;

use anyhow::{Context, Result};
use std::result::Result::Ok;

use crate::config_manager::Config;
use crate::daemon_communication::daemon_client::APIClient;
use crate::{
    config_manager::{ConfigManager, INTERCEPTOR_STDOUT_FILE},
    FILE_CACHE_DIR, PID_FILE, REPO_NAME, REPO_OWNER, STDERR_FILE, STDOUT_FILE,
};

pub fn clean_up_after_daemon() -> Result<()> {
    std::fs::remove_file(PID_FILE).context("Failed to remove pid file")?;
    std::fs::remove_file(STDOUT_FILE).context("Failed to remove stdout file")?;
    std::fs::remove_file(STDERR_FILE).context("Failed to remove stderr file")?;
    let _ = std::fs::remove_file(INTERCEPTOR_STDOUT_FILE).context("Failed to remove stdout file");
    std::fs::remove_dir_all(FILE_CACHE_DIR).context("Failed to remove cache directory")?;
    Ok(())
}

pub async fn print_config_info(api_client: &APIClient, config: &Config) -> Result<()> {
    let mut output = String::new();
    let _ = writeln!(&mut output, "\n\n===== Tracer Info =====\n");

    match api_client.send_info_request().await {
        Ok(info) => {
            writeln!(&mut output, "Daemon status: {}", "Running".green())?;

            if let Some(ref inner) = info.inner {
                writeln!(&mut output, "Service name: {}", inner.pipeline_name)?;
                writeln!(&mut output, "Run name: {}", inner.run_name)?;
                writeln!(&mut output, "Run ID: {}", inner.run_id)?;
                writeln!(&mut output, "Total Run Time: {}", inner.formatted_runtime())?;
            }
            writeln!(
                &mut output,
                "Recognized Processes({}): {}",
                info.watched_processes_count,
                info.watched_processes_preview()
            )?;
        }
        Err(e) => {
            writeln!(&mut output, "Daemon status: {}", "Stopped".red())?;
            writeln!(&mut output, "Error info: {:?}", e)?;
        }
    }

    // todo: take version from CLI
    writeln!(&mut output, "Daemon version: {}", env!("CARGO_PKG_VERSION"))?;

    writeln!(
        &mut output,
        "Grafana Workspace URL: {}",
        config.grafana_workspace_url.cyan().underline()
    )?;

    writeln!(
        &mut output,
        "Process polling interval: {} ms",
        config.process_polling_interval_ms
    )?;

    writeln!(
        &mut output,
        "Batch submission interval: {} ms",
        config.batch_submission_interval_ms
    )?;

    writeln!(&mut output, "\n===== ... =====\n\n")?;

    println!("{}", output);

    Ok(())
}

pub async fn setup_config(
    api_key: &Option<String>,
    process_polling_interval_ms: &Option<u64>,
    batch_submission_interval_ms: &Option<u64>,
) -> Result<()> {
    ConfigManager::modify_config(
        api_key,
        process_polling_interval_ms,
        batch_submission_interval_ms,
    )?;

    Ok(())
}

pub async fn update_tracer() -> Result<()> {
    let octocrab = octocrab::instance();

    let release = octocrab
        .repos(REPO_OWNER, REPO_NAME)
        .releases()
        .get_latest()
        .await?;

    if release.tag_name == env!("CARGO_PKG_VERSION") {
        println!("You are already using the latest version of Tracer.");
        return Ok(());
    }

    let config = ConfigManager::load_config();

    println!("Updating Tracer to version {}", release.tag_name);

    let mut command = Command::new("bash");
    command.arg("-c").arg(format!("curl -sSL https://raw.githubusercontent.com/davincios/tracer-daemon/main/install-tracer.sh | bash -s -- {} && . ~/.bashrc && tracer", config.api_key));

    command
        .status()
        .context("Failed to update Tracer. Please try again.")?;

    Ok(())
}
