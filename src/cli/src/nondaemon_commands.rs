use colored::Colorize;
use std::fmt::Write;
use std::process::Command;

use anyhow::{bail, Context, Result};
use std::result::Result::Ok;
use tokio::time::sleep;
use tracer_client::config_manager::{Config, ConfigLoader, INTERCEPTOR_STDOUT_FILE};
use tracer_common::constants::{
    FILE_CACHE_DIR, PID_FILE, REPO_NAME, REPO_OWNER, STDERR_FILE, STDOUT_FILE,
};
use tracer_daemon::client::DaemonClient;
use tracing::debug;

pub fn clean_up_after_daemon() -> Result<()> {
    std::fs::remove_file(PID_FILE).context("Failed to remove pid file")?;
    std::fs::remove_file(STDOUT_FILE).context("Failed to remove stdout file")?;
    std::fs::remove_file(STDERR_FILE).context("Failed to remove stderr file")?;
    let _ = std::fs::remove_file(INTERCEPTOR_STDOUT_FILE).context("Failed to remove stdout file");
    std::fs::remove_dir_all(FILE_CACHE_DIR).context("Failed to remove cache directory")?;
    Ok(())
}

pub async fn wait(api_client: &DaemonClient) -> Result<()> {
    for n in 0..5 {
        match api_client
            .client
            .get(api_client.get_url("/info"))
            .send()
            .await
        {
            // if timeout, retry
            Err(e) => {
                if !(e.is_timeout() || e.is_connect()) {
                    bail!(e)
                }
            }
            Ok(resp) => {
                if resp.status().is_success() {
                    return Ok(());
                }

                debug!("Got response, retrying: {:?}", resp);
            }
        }

        let duration = 1 << n;
        let attempts = match duration {
            1 => 1,
            2 => 2,
            4 => 3,
            8 => 4,
            _ => 5,
        };

        println!(
            "Starting daemon... [{:.<20}] ({} second{} elapsed)",
            ".".repeat(attempts.min(20)),
            duration,
            if duration > 1 { "s" } else { "" }
        );
        sleep(std::time::Duration::from_secs(duration)).await;
    }

    bail!("Daemon not started yet")
}

pub async fn print_config_info(api_client: &DaemonClient, config: &Config) -> Result<()> {
    let mut output = String::new();

    let info = match api_client.send_info_request().await {
        Ok(info) => info,
        Err(e) => {
            writeln!(&mut output, "Daemon status: {}\n", "Stopped".red())?;
            writeln!(
                &mut output,
                "To start the daemon run {}\n",
                "tracer init".cyan().bold()
            )?;
            writeln!(
                &mut output,
                "This error occured while trying to access the daemon info:\n\n{:?}",
                e
            )?;
            println!("{}", output);
            return Ok(());
        }
    };

    // Fixed width for the left column and separator
    let total_header_width = 80; // Reasonable width for the header

    writeln!(
        &mut output,
        "\n┌{:─^width$}┐",
        " TRACER INFO ",
        width = total_header_width
    )?;

    writeln!(
        &mut output,
        "│ Daemon status:            │ {}  ",
        "Running".green()
    )?;

    if let Some(ref inner) = info.inner {
        writeln!(
            &mut output,
            "│ Service name:             │ {}  ",
            inner.pipeline_name
        )?;
        writeln!(
            &mut output,
            "│ Run name:                 │ {}  ",
            inner.run_name
        )?;
        writeln!(
            &mut output,
            "│ Run ID:                   │ {}  ",
            inner.run_id
        )?;
        writeln!(
            &mut output,
            "│ Total Run Time:           │ {}  ",
            inner.formatted_runtime()
        )?;
    }
    writeln!(
        &mut output,
        "│ Recognized Processes:     │ {}:{}  ",
        info.watched_processes_count,
        info.watched_processes_preview()
    )?;

    writeln!(
        &mut output,
        "│ Daemon version:           │ {}  ",
        env!("CARGO_PKG_VERSION")
    )?;

    // Special case for Grafana URL - create clickable link with color
    let clickable_url = format!(
        "\u{1b}]8;;{0}\u{1b}\\{0}\u{1b}]8;;\u{1b}\\",
        config.grafana_workspace_url
    );
    let colored_url = clickable_url.cyan().underline().to_string();

    writeln!(
        &mut output,
        "│ Grafana Workspace URL:    │ {}  ",
        colored_url
    )?;

    let config_sources = if config.config_sources.is_empty() {
        vec!["No config file used".to_string()]
    } else {
        config.config_sources.clone()
    };
    if let Some((first, rest)) = config_sources.split_first() {
        writeln!(&mut output, "│ Config Sources:           │ {}  ", first)?;
        for source in rest {
            writeln!(&mut output, "│                           │ {}  ", source)?;
        }
    }

    writeln!(
        &mut output,
        "│ Process polling interval: │ {} ms  ",
        config.process_polling_interval_ms
    )?;

    writeln!(
        &mut output,
        "│ Batch submission interval:│ {} ms  ",
        config.batch_submission_interval_ms
    )?;

    writeln!(
        &mut output,
        "│ Log files:                │ {}  ",
        STDOUT_FILE
    )?;

    writeln!(
        &mut output,
        "│                           │ {}  ",
        STDERR_FILE
    )?;

    writeln!(&mut output, "└{:─^width$}┘", "", width = total_header_width)?;

    println!("{}", output);

    Ok(())
}

pub async fn setup_config(
    api_key: &Option<String>,
    process_polling_interval_ms: &Option<u64>,
    batch_submission_interval_ms: &Option<u64>,
) -> Result<()> {
    let mut current_config = ConfigLoader::load_config(None)?;
    if let Some(api_key) = api_key {
        current_config.api_key.clone_from(api_key);
    }
    if let Some(process_polling_interval_ms) = process_polling_interval_ms {
        current_config.process_polling_interval_ms = *process_polling_interval_ms;
    }
    if let Some(batch_submission_interval_ms) = batch_submission_interval_ms {
        current_config.batch_submission_interval_ms = *batch_submission_interval_ms;
    }
    ConfigLoader::save_config(&current_config)?;

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

    let config = ConfigLoader::load_config(None)?;

    println!("Updating Tracer to version {}", release.tag_name);

    let mut command = Command::new("bash");
    command.arg("-c").arg(format!("curl -sSL https://raw.githubusercontent.com/davincios/tracer-daemon/main/install-tracer.sh | bash -s -- {} && . ~/.bashrc && tracer", config.api_key));

    command
        .status()
        .context("Failed to update Tracer. Please try again.")?;

    Ok(())
}
