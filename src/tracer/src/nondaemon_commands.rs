use std::process::Command;

#[cfg(target_os = "linux")]
use crate::utils::system_info::get_kernel_version;

use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::process_identification::constants::{
    FILE_CACHE_DIR, PID_FILE, STDERR_FILE, STDOUT_FILE,
};
use crate::utils::InfoDisplay;
use anyhow::{bail, Context, Result};
use colored::Colorize;
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
        match api_client.send_info().await {
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

pub async fn print_config_info(
    api_client: &DaemonClient,
    config: &Config,
    json: bool,
) -> Result<()> {
    let mut display = InfoDisplay::new(70, json);
    let info = match api_client.send_info_request().await {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Error getting info response: {e}");
            display.print_error();
            return Ok(());
        }
    };
    display.print(info, config);
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

pub async fn update_tracer() -> Result<()> {
    // TODO commenting out for now, as we get the s3 main release
    // let octocrab = octocrab::instance();
    // let release = octocrab
    //     .repos(REPO_OWNER, REPO_NAME)
    //     .releases()
    //     .get_latest()
    //     .await?;

    // let current = Version::current_str();
    // let latest = &release.tag_name;

    // let current_ver: Version = current.parse().ok().unwrap();
    // let latest_ver: Version = latest.parse().ok().unwrap();
    //
    // if latest_ver <= current_ver {
    //     println!(
    //         "\nTracer is already at the latest version: {}.",
    //         current_ver
    //     );
    //     return Ok(());
    // }

    // println!("\nA new version of Tracer is available!");
    // println!("\nVersion Information:");
    // println!("  Current Version: {}", current_ver);
    // println!("  Latest Version:  {}", latest_ver);
    //
    // println!("\nWould you like to proceed with the update? [y/N]");
    // let mut input = String::new();
    // std::io::stdin().read_line(&mut input)?;
    //
    // if !input.trim().eq_ignore_ascii_case("y") {
    //     println!("Update cancelled by user.");
    //     return Ok(());
    // }
    //
    // println!("\nUpdating Tracer to version {}...", latest_ver);

    let mut command = Command::new("bash");
    command.arg("-c").arg("curl -sSL https://install.tracer.cloud/ | sudo bash && source ~/.bashrc && source ~/.zshrc");
    let status = command
        .status()
        .context("Failed to update Tracer. Please try again.")?;

    if !status.success() {
        bail!("Failed to update Tracer. Please try again.");
    }

    println!(
        "\n{} Tracer has been successfully updated!",
        "Success:".green(),
        // latest_ver
    );

    // println!(
    //     "\n{} Tracer has been successfully updated to version {}!",
    //     "Success:".green(),
    //     latest_ver
    // );
    Ok(())
}
