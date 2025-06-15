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

const STATUS_ACTIVE: Emoji<'_, '_> = Emoji("🟢 ", "🟢 ");
const STATUS_INACTIVE: Emoji<'_, '_> = Emoji("🔴 ", "🔴 ");
const STATUS_WARNING: Emoji<'_, '_> = Emoji("🟡 ", "🟡 ");
const STATUS_INFO: Emoji<'_, '_> = Emoji("ℹ️ ", "ℹ️ ");

struct InfoFormatter {
    output: String,
    width: usize,
}

impl InfoFormatter {
    fn new(width: usize) -> Self {
        Self {
            output: String::new(),
            width,
        }
    }

    fn add_header(&mut self, title: &str) -> Result<()> {
        writeln!(
            &mut self.output,
            "\n┌{:─^width$}┐",
            format!(" {} ", title),
            width = self.width - 2
        )?;
        Ok(())
    }

    fn add_footer(&mut self) -> Result<()> {
        writeln!(
            &mut self.output,
            "└{:─^width$}┘",
            "",
            width = self.width - 2
        )?;
        Ok(())
    }

    fn add_section_header(&mut self, title: &str) -> Result<()> {
        writeln!(
            &mut self.output,
            "├{:─^width$}┤",
            format!(" {} ", title),
            width = self.width - 2
        )?;
        Ok(())
    }

    fn add_field(&mut self, label: &str, value: &str, color: &str) -> Result<()> {
        let colored_value = match color {
            "green" => value.green(),
            "yellow" => value.yellow(),
            "cyan" => value.cyan(),
            "magenta" => value.magenta(),
            "blue" => value.blue(),
            "red" => value.red(),
            "bold" => value.bold(),
            _ => value.normal(),
        };

        // Calculate available space for value
        let label_width = 20;
        let padding = 4;
        let max_value_width = self.width - label_width - padding;

        // Format the value with proper truncation
        let formatted_value = if colored_value.len() > max_value_width {
            format!("{}...", &colored_value[..max_value_width - 3])
        } else {
            colored_value.to_string()
        };

        writeln!(
            &mut self.output,
            "│ {:<label_width$} │ {}  ",
            label, formatted_value
        )?;
        Ok(())
    }

    fn add_status_field(&mut self, label: &str, value: &str, status: &str) -> Result<()> {
        let (emoji, color) = match status {
            "active" => (STATUS_ACTIVE, "green"),
            "inactive" => (STATUS_INACTIVE, "red"),
            "warning" => (STATUS_WARNING, "yellow"),
            _ => (STATUS_INFO, "blue"),
        };

        writeln!(
            &mut self.output,
            "│ {:<20} │ {} {}  ",
            label,
            emoji,
            value.color(color)
        )?;
        Ok(())
    }

    fn add_empty_line(&mut self) -> Result<()> {
        writeln!(&mut self.output, "│{:width$}│", "", width = self.width - 2)?;
        Ok(())
    }
}

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
    let mut formatter = InfoFormatter::new(90);
    let info = match api_client.send_info_request().await {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Error getting info response: {e}");
            print_error_state(&mut formatter)?;
            println!("{}", formatter.output);
            return Ok(());
        }
    };

    formatter.add_header("TRACER INFO")?;
    formatter.add_empty_line()?;

    print_daemon_status(&mut formatter)?;

    if let Some(inner) = &info.inner {
        print_pipeline_info(&mut formatter, inner, &info)?;
    }

    print_config_and_logs(&mut formatter, config)?;
    formatter.add_footer()?;
    println!("{}", formatter.output);
    Ok(())
}

fn print_error_state(formatter: &mut InfoFormatter) -> Result<()> {
    formatter.add_header("TRACER CLI STATUS")?;
    formatter.add_empty_line()?;
    formatter.add_status_field("Daemon Status", "Not Started", "inactive")?;
    formatter.add_field("Version", env!("CARGO_PKG_VERSION"), "bold")?;
    formatter.add_empty_line()?;
    formatter.add_section_header("NEXT STEPS")?;
    formatter.add_empty_line()?;
    formatter.add_field("Interactive Setup", "tracer init", "bold")?;
    formatter.add_field("Visualize Data", "https://sandbox.tracer.app", "blue")?;
    formatter.add_field(
        "Documentation",
        "https://github.com/Tracer-Cloud/tracer-client",
        "blue",
    )?;
    formatter.add_field("Support", "support@tracer.cloud", "blue")?;
    formatter.add_empty_line()?;
    formatter.add_footer()?;
    Ok(())
}

fn print_daemon_status(formatter: &mut InfoFormatter) -> Result<()> {
    formatter.add_section_header("DAEMON STATUS")?;
    formatter.add_empty_line()?;
    formatter.add_status_field("Status", "Running", "active")?;
    formatter.add_field("Version", env!("CARGO_PKG_VERSION"), "bold")?;
    formatter.add_empty_line()?;
    Ok(())
}

fn print_pipeline_info(
    formatter: &mut InfoFormatter,
    inner: &crate::daemon::structs::InnerInfoResponse,
    info: &crate::daemon::structs::InfoResponse,
) -> Result<()> {
    formatter.add_section_header("RUN DETAILS")?;
    formatter.add_empty_line()?;

    // Pipeline section
    formatter.add_field("Pipeline Name", &inner.pipeline_name, "bold")?;
    formatter.add_field(
        "Pipeline Type",
        inner.tags.pipeline_type.as_deref().unwrap_or("Not Set"),
        "cyan",
    )?;
    formatter.add_field(
        "Environment",
        inner.tags.environment.as_deref().unwrap_or("Not Set"),
        "yellow",
    )?;
    formatter.add_field(
        "User",
        inner.tags.user_operator.as_deref().unwrap_or("Not Set"),
        "magenta",
    )?;

    // Run section
    formatter.add_field("Run Name", &inner.run_name, "bold")?;
    formatter.add_field("Run ID", &inner.run_id, "cyan")?;
    formatter.add_field("Runtime", &inner.formatted_runtime(), "yellow")?;
    formatter.add_field(
        "Monitored Processes",
        &format!(
            "{}: {}",
            info.watched_processes_count.to_string().bold(),
            info.watched_processes_preview().cyan()
        ),
        "normal",
    )?;
    formatter.add_empty_line()?;
    Ok(())
}

fn print_config_and_logs(formatter: &mut InfoFormatter, config: &Config) -> Result<()> {
    formatter.add_section_header("CONFIGURATION & LOGS")?;
    formatter.add_empty_line()?;

    let grafana_url = if config.grafana_workspace_url.is_empty() {
        "Not configured".to_string()
    } else {
        config.grafana_workspace_url.clone()
    };

    formatter.add_field("Grafana Workspace", &format!("{} ", grafana_url), "blue")?;
    formatter.add_field(
        "Process Polling",
        &format!("{} ms ", config.process_polling_interval_ms),
        "yellow",
    )?;
    formatter.add_field(
        "Batch Submission",
        &format!("{} ms ", config.batch_submission_interval_ms),
        "yellow",
    )?;
    formatter.add_field("Standard Output", &format!("{} ", STDOUT_FILE), "cyan")?;
    formatter.add_field("Standard Error", &format!("{} ", STDERR_FILE), "cyan")?;
    formatter.add_field("Log File", &format!("{} ", LOG_FILE), "cyan")?;
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
