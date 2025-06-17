use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "linux")]
use crate::utils::get_kernel_version;

use crate::common::constants::{ARGS_DIR, FILE_CACHE_DIR, PID_FILE, REPO_NAME, REPO_OWNER, STDERR_FILE, STDOUT_FILE};
use crate::common::types::pipeline_tags::PipelineTags;
use crate::config::Config;
use crate::daemon;
use crate::daemon::client::DaemonClient;
use crate::utils::info_formatter::InfoFormatter;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::result::Result::Ok;
use tokio::time::sleep;
use tracing::debug;

pub fn clean_up_after_daemon() -> Result<()> {
    std::fs::remove_file(PID_FILE).context("Failed to remove pid file")?;
    std::fs::remove_file(STDOUT_FILE).context("Failed to remove stdout file")?;
    std::fs::remove_file(STDERR_FILE).context("Failed to remove stderr file")?;
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
            formatter.print_error_state()?;
            println!("{}", formatter.get_output());
            return Ok(());
        }
    };

    formatter.add_header("TRACER INFO")?;
    formatter.add_empty_line()?;

    formatter.print_daemon_status()?;

    if let Some(inner) = &info.inner {
        formatter.print_pipeline_info(inner, &info)?;
    }

    formatter.print_config_and_logs(config)?;
    formatter.add_footer()?;
    println!("{}", formatter.get_output());
    Ok(())
}

pub async fn setup_config(
    api_key: &Option<String>,
    process_polling_interval_ms: &Option<u64>,
    batch_submission_interval_ms: &Option<u64>,
) -> Result<()> {
    let mut current_config = Config::default();
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


/// Represents the running daemon state that needs to be preserved during updates
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaemonState {
    run_id: String,
    run_name: String,
    pipeline_name: String,
    tags: PipelineTags,
    is_dev: bool,
    init_args: Vec<String>, // initial arguments passed to tracer init
}

impl DaemonState {
    /// Creates a new DaemonState from the current daemon info
    fn from_daemon_info(
        inner: &daemon::structs::InnerInfoResponse,
        config: &Config,
        init_args: Vec<String>,
    ) -> Self {
        // Extract only the init arguments, skipping the binary path and 'update' command
        let filtered_args: Vec<String> = init_args
            .into_iter()
            .skip_while(|arg| arg.contains("tracer_cli") || arg == "update")
            .collect();

        // Create tags from the arguments
        let mut tags = PipelineTags::default();
        let mut args_iter = filtered_args.iter().peekable();

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "--environment" => {
                    if let Some(value) = args_iter.next() {
                        tags.environment = Some(value.clone());
                    }
                }
                "--pipeline-type" => {
                    if let Some(value) = args_iter.next() {
                        tags.pipeline_type = Some(value.clone());
                    }
                }
                "--user-operator" => {
                    if let Some(value) = args_iter.next() {
                        tags.user_operator = Some(value.clone());
                    }
                }
                "--department" => {
                    if let Some(value) = args_iter.next() {
                        tags.department = value.clone();
                    }
                }
                "--team" => {
                    if let Some(value) = args_iter.next() {
                        tags.team = value.clone();
                    }
                }
                "--organization-id" => {
                    if let Some(value) = args_iter.next() {
                        tags.organization_id = Some(value.clone());
                    }
                }
                _ => {}
            }
        }

        Self {
            run_id: inner.run_id.clone(),
            run_name: inner.run_name.clone(),
            pipeline_name: inner.pipeline_name.clone(),
            tags,
            is_dev: config.log_forward_endpoint_dev.is_some(),
            init_args: filtered_args,
        }
    }

    /// Gets the args directory path
    fn get_args_dir() -> Result<PathBuf> {
        let args_dir = PathBuf::from(ARGS_DIR);
        std::fs::create_dir_all(&args_dir)?;
        Ok(args_dir)
    }

    /// Saves the daemon state to a file
    fn save(&self) -> Result<()> {
        let args_dir = Self::get_args_dir()?;
        std::fs::create_dir_all(&args_dir)?;

        // Save the state
        let state_file = args_dir.join("daemon_state.json");
        let state_json = serde_json::to_string_pretty(self)?;
        std::fs::write(&state_file, state_json)?;

        // Save the init args separately for easy access
        let args_file = args_dir.join("init_args.json");
        let args_json = serde_json::to_string_pretty(&self.init_args)?;
        std::fs::write(&args_file, args_json)?;

        // Set file permissions
        std::fs::set_permissions(&state_file, std::fs::Permissions::from_mode(0o644))?;
        std::fs::set_permissions(&args_file, std::fs::Permissions::from_mode(0o644))?;

        Ok(())
    }

    /// Loads the daemon state from the args dir
    fn load() -> Result<Option<Self>> {
        let args_dir = Self::get_args_dir()?;
        let state_file = args_dir.join("daemon_state.json");
        let args_file = args_dir.join("init_args.json");

        if !state_file.exists() || !args_file.exists() {
            return Ok(None);
        }

        let state_json = std::fs::read_to_string(state_file)?;
        let args_json = std::fs::read_to_string(args_file)?;

        let mut state: Self = serde_json::from_str(&state_json)?;
        state.init_args = serde_json::from_str(&args_json)?;

        Ok(Some(state))
    }

    /// Removes the saved daemon state
    async fn cleanup() -> Result<()> {
        let args_dir = Self::get_args_dir()?;
        let state_file = args_dir.join("daemon_state.json");
        let args_file = args_dir.join("init_args.json");

        // Only cleanup if both files exist
        if state_file.exists() && args_file.exists() {
            // Check if daemon is running
            let config = Config::default();
            let api_client = DaemonClient::new(format!("http://{}", config.server));

            // If we can't connect to the daemon, it's safe to cleanup
            if api_client.send_info_request().await.is_err() {
                if state_file.exists() {
                    std::fs::remove_file(state_file)?;
                }
                if args_file.exists() {
                    std::fs::remove_file(args_file)?;
                }
            }
        }
        Ok(())
    }

    /// Creates a command to restart the daemon with the saved state
    fn create_restart_command(&self) -> Command {
        let mut cmd = Command::new("tracer");
        cmd.arg("init");

        cmd.arg("--pipeline-name").arg(&self.pipeline_name);

        if let Some(environment) = &self.tags.environment {
            cmd.arg("--environment").arg(environment);
        }
        if let Some(pipeline_type) = &self.tags.pipeline_type {
            cmd.arg("--pipeline-type").arg(pipeline_type);
        }
        if let Some(user_operator) = &self.tags.user_operator {
            cmd.arg("--user-operator").arg(user_operator);
        }
        if !self.tags.department.is_empty() {
            cmd.arg("--department").arg(&self.tags.department);
        }
        if !self.tags.team.is_empty() {
            cmd.arg("--team").arg(&self.tags.team);
        }
        if let Some(org_id) = &self.tags.organization_id {
            cmd.arg("--organization-id").arg(org_id);
        }

        cmd.arg("--is-dev")
            .arg(if self.is_dev { "true" } else { "false" });

        cmd.arg("--run-id").arg(&self.run_id);

        cmd
    }

    /// Displays the daemon state and configuration information
    fn display_info(&self, config: &Config, show_restart_command: bool) {
        println!("\n{} Daemon Configuration:", "Info:".cyan());
        println!(
            "  Daemon Config Directory: {}",
            Path::new(&ARGS_DIR).join("daemon_state.json").display()
        );
        println!("  Server: {}", config.server);

        println!("  Run ID: {}", self.run_id);
        println!("  Run Name: {}", self.run_name);
        println!("  Pipeline: {}", self.pipeline_name);
        println!(
            "  Process Polling Interval: {} ms",
            config.process_polling_interval_ms
        );
        println!(
            "  Batch Submission Interval: {} ms",
            config.batch_submission_interval_ms
        );

        println!(
            "  Environment: {}",
            self.tags.environment.as_deref().unwrap_or("unknown")
        );
        println!(
            "  Pipeline Type: {}",
            self.tags.pipeline_type.as_deref().unwrap_or("unknown")
        );
        println!(
            "  User Operator: {}",
            self.tags.user_operator.as_deref().unwrap_or("unknown")
        );
        println!("  Department: {}", self.tags.department);
        println!("  Team: {}", self.tags.team);
        if let Some(org_id) = &self.tags.organization_id {
            println!("  Organization ID: {}", org_id);
        }
        println!("  Grafana Workspace URL: {}", config.grafana_workspace_url);

        println!("\nThe daemon will be restarted with these settings after the update.");

        if show_restart_command {
            println!("\nRestart command will be:");
            let cmd = self.create_restart_command();
            println!("{}", format!("{:?}", cmd).replace("\"", ""));
        }
    }
}


fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim_start_matches('v');
    let main_version = s.split('+').next()?;
    let parts: Vec<&str> = main_version.split('.').collect();

    if parts.len() != 3 {
        return None;
    }

    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    let patch = parts[2].parse::<u32>().ok()?;

    Some((major, minor, patch))
}

fn format_version(ver: (u32, u32, u32)) -> String {
    format!("{}.{}.{}", ver.0, ver.1, ver.2)
}

pub async fn update_tracer() -> Result<()> {
    let octocrab = octocrab::instance();
    let release = octocrab
        .repos(REPO_OWNER, REPO_NAME)
        .releases()
        .get_latest()
        .await?;

    let current = env!("CARGO_PKG_VERSION");
    let latest = &release.tag_name;

    let current_ver = parse_version(current)
        .ok_or_else(|| anyhow::anyhow!("Invalid current version format: {}", current))?;
    let latest_ver = parse_version(latest)
        .ok_or_else(|| anyhow::anyhow!("Invalid latest version format: {}", latest))?;

    if latest_ver <= current_ver {
        println!(
            "\nTracer is already at the latest version: {}.",
            format_version(current_ver)
        );
        return Ok(());
    }

    // Get current daemon state
    let config = Config::default();
    let api_client = DaemonClient::new(format!("http://{}", config.server));
    let daemon_info = api_client.send_info_request().await;
    let is_daemon_running = daemon_info.is_ok();
    let mut daemon_state = None;


    println!("\nA new version of Tracer is available!");
    println!("\nVersion Information:");
    println!("  Current Version: {}", format_version(current_ver));
    println!("  Latest Version:  {}", format_version(latest_ver));

    if is_daemon_running {
        if let Ok(info) = daemon_info {
            if let Some(inner) = info.inner {
                println!("\n{} A daemon is currently running. It is recommended to update when no flows are active.", "Warning:".yellow());
                println!("Current run details:");
                println!("  Run ID: {}", inner.run_id);
                println!("  Run Name: {}", inner.run_name);
                println!("  Pipeline: {}", inner.pipeline_name);
                println!("\nWould you like to proceed with the update anyway? [y/N] ");

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Update cancelled.");
                    return Ok(());
                }

                // Get the current command line arguments
                let init_args = std::env::args().collect::<Vec<String>>();

                // Save daemon state with init args
                daemon_state = Some(DaemonState::from_daemon_info(&inner, &config, init_args));
                daemon_state.as_ref().unwrap().display_info(&config, false);
            }
        }
    }

    println!("\nWould you like to proceed with the update? [y/N]");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Update cancelled by user.");
        return Ok(());
    }

    if is_daemon_running {
        println!("\nStopping the daemon...");
        api_client.send_terminate_request().await?;
        clean_up_after_daemon()?;
    }

    let config = Config::default();

    println!(
        "\nUpdating Tracer to version {}...",
        format_version(latest_ver)
    );

    let mut command = Command::new("bash");
    command.arg("-c").arg(format!(
        "curl -sSL https://install.tracer.cloud | bash -s -- {} && . ~/.bashrc",
        config.api_key
    ));

    let status = command
        .status()
        .context("Failed to update Tracer. Please try again.")?;

    if !status.success() {
        bail!("Failed to update Tracer. Please try again.");
    }

    println!(
        "\n{} Tracer has been successfully updated to version {}!",
        "Success:".green(),
        format_version(latest_ver)
    );

    // Save daemon state if it was running
    if let Some(state) = daemon_state {
        state.save()?;
    }

    // Restart the daemon if it was running
    if let Some(daemon_state) = DaemonState::load()? {
        println!("\n{} Restarting the daemon", "Info:".cyan());

        let mut restart_cmd = daemon_state.create_restart_command();

        match restart_cmd.status() {
            Ok(status) => {
                if status.success() {
                    println!("Daemon restarted successfully.");

                    sleep(std::time::Duration::from_secs(2)).await;

                    let new_config = Config::default();
                    let new_api_client = DaemonClient::new(format!("http://{}", new_config.server));

                    if let Ok(info) = new_api_client.send_info_request().await {
                        println!("\n{} Successfully Restarted Daemon:", "Success:".green());
                        if let Some(inner) = info.inner {
                            let new_state =
                                DaemonState::from_daemon_info(&inner, &new_config, vec![]);

                            // Save the new state
                            new_state.save()?;
                        }
                    }
                } else {
                    println!("{} Failed to restart daemon. Please run 'tracer init' manually with the following command:", "Warning:".yellow());
                    daemon_state.display_info(&config, true);
                }
            }
            Err(e) => {
                println!("{} Failed to restart daemon: {}. Please run 'tracer init' manually with the following command:", "Warning:".yellow(), e);
                daemon_state.display_info(&config, true);
            }
        }
    }

    Ok(())
}
