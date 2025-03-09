use colored::Colorize;
use std::fmt::Write;
use std::process::Command;

use anyhow::{Context, Result};
use std::result::Result::Ok;

use crate::{
    config_manager::{ConfigManager, INTERCEPTOR_STDOUT_FILE},
    daemon_communication::client::{send_info_request, send_refresh_config_request},
    FILE_CACHE_DIR, PID_FILE, REPO_NAME, REPO_OWNER, SOCKET_PATH, STDERR_FILE, STDOUT_FILE,
};

pub fn clean_up_after_daemon() -> Result<()> {
    std::fs::remove_file(PID_FILE).context("Failed to remove pid file")?;
    std::fs::remove_file(STDOUT_FILE).context("Failed to remove stdout file")?;
    std::fs::remove_file(STDERR_FILE).context("Failed to remove stderr file")?;
    let _ = std::fs::remove_file(INTERCEPTOR_STDOUT_FILE).context("Failed to remove stdout file");
    std::fs::remove_dir_all(FILE_CACHE_DIR).context("Failed to remove cache directory")?;
    Ok(())
}

pub async fn print_config_info() -> Result<()> {
    let config = ConfigManager::load_config();
    let daemon_status = send_info_request(SOCKET_PATH).await;

    let mut output = String::new();

    let _ = writeln!(&mut output, "\n\n===== Tracer Info =====\n");

    if daemon_status.is_ok() {
        let _ = writeln!(&mut output, "Daemon status: {}", "Running".green());
    } else {
        let _ = writeln!(&mut output, "Daemon status: {}", "Stopped".red());
    }
    let _ = writeln!(&mut output, "Daemon version: {}", env!("CARGO_PKG_VERSION"));

    if let Ok(info) = daemon_status {
        if !info.run_name.is_empty() {
            let _ = writeln!(&mut output, "Service name: {}", info.pipeline_name);
            let _ = writeln!(&mut output, "Run name: {}", info.run_name);
            let _ = writeln!(&mut output, "Run ID: {}", info.run_id);
            // TODO: add total run time
        }
    }
    let _ = writeln!(
        &mut output,
        "Grafana Workspace URL: {}",
        config.grafana_workspace_url
    );

    let _ = writeln!(
        &mut output,
        "Process polling interval: {} ms",
        config.process_polling_interval_ms
    );

    let _ = writeln!(
        &mut output,
        "Batch submission interval: {} ms",
        config.batch_submission_interval_ms
    );

    let _ = writeln!(&mut output, "\n===== ... =====\n\n");

    println!("{}", output);

    Ok(())
}

pub fn print_config_info_sync() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(print_config_info())?;
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

    let _ = send_refresh_config_request(SOCKET_PATH).await;
    print_config_info().await?;
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
