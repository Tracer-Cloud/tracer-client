use colored::Colorize;
use console::Emoji;
use std::fmt::Write;
use std::process::Command;

#[cfg(target_os = "linux")]
use crate::utils::get_kernel_version;

use crate::client::config_manager::{Config, ConfigLoader, INTERCEPTOR_STDOUT_FILE};
use crate::common::constants::{
    FILE_CACHE_DIR, LOG_FILE, PID_FILE, REPO_NAME, REPO_OWNER, STDERR_FILE, STDOUT_FILE,
};
use crate::daemon::client::DaemonClient;
use anyhow::{bail, Context, Result};
use std::result::Result::Ok;
use tokio::time::sleep;
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

pub fn print_install_readiness() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let mut diagnostics: Vec<String> = vec![];
        let mut missing_packages: Vec<String> = vec![];
        let mut missing_package_advice: Vec<String> = vec![];

        let packages = [
            (
                "build-essential",
                "dpkg -s build-essential",
                "apt-get:build-essential",
            ),
            ("pkg-config", "dpkg -s pkg-config", "apt-get:pkg-config"),
            ("libelf1", "dpkg -s libelf1", "apt-get:libelf1"),
            ("libelf-dev", "dpkg -s libelf-dev", "apt-get:libelf-dev"),
            ("zlib1g-dev", "dpkg -s zlib1g-dev", "apt-get:zlib1g-dev"),
            ("llvm", "dpkg -s llvm", "apt-get:llvm"),
            ("clang", "dpkg -s clang", "apt-get:clang"),
        ];

        for (package_name, check_cmd, install_advice) in &packages {
            let is_installed = Command::new("sh")
                .arg("-c")
                .arg(check_cmd)
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false);

            if !is_installed {
                missing_packages.push(package_name.to_string());
                missing_package_advice.push(install_advice.to_string());
            }
        }

        if !missing_packages.is_empty() {
            let mut message = format!(
                "Found missing packages: {}\n\nTo install them run:\n",
                missing_packages.join(", ")
            );

            let mut apt_get_packages = vec![];
            let mut cargo_packages = vec![];

            for (package_name, _, install_advice) in &packages {
                if missing_packages.contains(&package_name.to_string()) {
                    if install_advice.starts_with("apt-get:") {
                        apt_get_packages.push(install_advice.replace("apt-get:", ""));
                    } else if install_advice.starts_with("cargo:") {
                        cargo_packages.push(install_advice.replace("cargo:", ""));
                    }
                }
            }

            if !apt_get_packages.is_empty() {
                message.push_str(&format!(
                    "sudo apt-get install -y {}\n",
                    apt_get_packages.join(" ")
                ));
            }

            if !cargo_packages.is_empty() {
                message.push_str(&format!("cargo install {}\n", cargo_packages.join(" ")));
            }

            diagnostics.push(message);
        }

        // Check kernel version (should be v5.15)
        let kernel_version = get_kernel_version();

        match kernel_version {
            Some((5, 15)) => {
                // Kernel version matches
            }
            Some((_major, _minor)) => {
                diagnostics.push(format!(
                    "Tracer has been tested and confirmed to work on Linux kernel v5.15, detected v{}.{}. Contact support if issues arise.",
                    _major, _minor
                ));
            }
            None => {
                diagnostics.push("Linux kernel version unknown. Recommended: v5.15.".to_string());
            }
        }

        // Print all collected diagnostics
        for warning in &diagnostics {
            println!();
            println!("{}", warning);
        }
        if !&diagnostics.is_empty() {
            println!();
        }
    }

    #[cfg(target_os = "macos")]
    {
        println!("Detected MacOS. eBPF is not supported on MacOS.");
        println!("Activated process polling \n");
    }

    Ok(())
}

pub async fn print_config_info(api_client: &DaemonClient, config: &Config) -> Result<()> {
    let mut output = String::new();

    let info = match api_client.send_info_request().await {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Error getting info response: {e}");
            const CHECK: Emoji<'_, '_> = Emoji("✨ ", "[OK] ");
            const PLAY: Emoji<'_, '_> = Emoji("▶️ ", "▶ ");
            const BOOK: Emoji<'_, '_> = Emoji("📖 ", "-> ");
            const SUPPORT: Emoji<'_, '_> = Emoji("✉️ ", "-> ");
            const WEB: Emoji<'_, '_> = Emoji("🌐 ", "-> ");
            const WARNING: Emoji<'_, '_> = Emoji("⚠️ ", "⚠ ");
            let width = 75;

            writeln!(
                &mut output,
                "\n{} {}",
                CHECK,
                "Tracer CLI installed.".bold()
            )?;
            writeln!(
                &mut output,
                "{} Daemon status: {}",
                WARNING,
                "Not started yet".yellow()
            )?;

            writeln!(
                &mut output,
                "\n   ╭{:─^width$}╮",
                " Next Steps ",
                width = width
            )?;
            writeln!(&mut output, "   │{:width$}│", "", width = width)?;

            writeln!(
                &mut output,
                "   │ {} {:<width$} │",
                PLAY,
                "tracer init         Interactive Pipeline Setup",
                width = width - 6
            )?;
            writeln!(&mut output, "   │{:width$}│", "", width = width)?;

            writeln!(
                &mut output,
                "   │ {} Visualize Data:     {:<width$}                        │",
                WEB,
                "https://sandbox.tracer.cloud".bright_blue().underline(),
                width = width - 50
            )?;
            writeln!(&mut output, "   │{:width$}│", "", width = width)?;

            writeln!(
                &mut output,
                "   │ {} Documentation:      {:<width$}     │",
                BOOK,
                "https://github.com/Tracer-Cloud/tracer-client"
                    .bright_blue()
                    .underline(),
                width = width - 30
            )?;
            writeln!(&mut output, "   │{:width$}│", "", width = width)?;

            writeln!(
                &mut output,
                "   │ {} Support: {:<width$} │",
                SUPPORT,
                "           support@tracer.cloud".bright_blue(),
                width = width - 15
            )?;
            writeln!(&mut output, "   │{:width$}│", "", width = width)?;

            writeln!(&mut output, "   ╰{:─^width$}╯", "", width = width)?;
            println!("{}", output);
            return Ok(());
        }
    };

    // Fixed width for the left column and separator
    let total_header_width = 80;

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
            "│ Pipeline name:            │ {}  ",
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

    let clickable_url = format!(
        "\u{1b}]8;;{0}\u{1b}\\{0}\u{1b}]8;;\u{1b}\\",
        config.grafana_workspace_url
    );
    let colored_url = clickable_url.bright_blue().underline().to_string();

    writeln!(
        &mut output,
        "│ Grafana Workspace URL:    │ {}  ",
        colored_url
    )?;

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
        "│ Tracer Agent Log files:   │ {}  ",
        STDOUT_FILE
    )?;

    writeln!(
        &mut output,
        "│                           │ {}  ",
        STDERR_FILE
    )?;

    writeln!(&mut output, "│                           │ {}  ", LOG_FILE)?;

    writeln!(&mut output, "└{:─^width$}┘", "", width = total_header_width)?;

    println!("{}", output);
    Ok(())
}
pub async fn setup_config(
    api_key: &Option<String>,
    process_polling_interval_ms: &Option<u64>,
    batch_submission_interval_ms: &Option<u64>,
) -> Result<()> {
    let mut current_config = ConfigLoader::load_default_config()?;
    if let Some(api_key) = api_key {
        current_config.api_key.clone_from(api_key);
    }
    if let Some(process_polling_interval_ms) = process_polling_interval_ms {
        current_config.process_polling_interval_ms = *process_polling_interval_ms;
    }
    if let Some(batch_submission_interval_ms) = batch_submission_interval_ms {
        current_config.batch_submission_interval_ms = *batch_submission_interval_ms;
    }

    //ConfigLoader::save_config(&current_config)?;

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

    let config = ConfigLoader::load_default_config()?;

    println!("Updating Tracer to version {}", release.tag_name);

    let mut command = Command::new("bash");
    command.arg("-c").arg(format!("curl -sSL https://raw.githubusercontent.com/davincios/tracer-daemon/main/install-tracer.sh | bash -s -- {} && . ~/.bashrc && tracer", config.api_key));

    command
        .status()
        .context("Failed to update Tracer. Please try again.")?;

    Ok(())
}
