use colored::Colorize;
use console::Emoji;
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
use semver;

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
    let kernel_version = Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8(output.stdout).ok().and_then(|version| {
                let parts: Vec<&str> = version.trim().split('.').collect();
                if parts.len() >= 2 {
                    let major = parts[0].parse::<u32>().ok()?;
                    let minor = parts[1].parse::<u32>().ok()?;
                    Some((major, minor))
                } else {
                    None
                }
            })
        });

    match kernel_version {
        Some((5, 15)) => {
            // Kernel version matches
        }
        Some((major, minor)) => {
            diagnostics.push(format!(
                "Tracer has been tested and confirmed to work on Linux kernel v5.15, detected v{}.{}. Contact support if issues arise.",
                major, minor
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

    Ok(())
}

pub async fn print_config_info(api_client: &DaemonClient, config: &Config) -> Result<()> {
    let mut output = String::new();

    let info = match api_client.send_info_request().await {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Error getting info response: {e}");
            const NEXT: Emoji<'_, '_> = Emoji("⏭️", "->");
            const CHECK: Emoji<'_, '_> = Emoji("✅", " ");

            writeln!(&mut output, "Daemon status: {}\n", "Stopped".red())?;

            //

            writeln!(
                &mut output,
                "\n{} {}",
                CHECK,
                "Tracer agent installed successfully:".bold()
            )?;

            writeln!(
                &mut output,
                "\n{} {}",
                NEXT,
                "Initialize the Tracer agent by running:".bold()
            )?;
            writeln!(
            &mut output,
            "\n{}\n",
            "tracer init --pipeline-name demo_username --environment demo --pipeline-type rnaseq --user-operator user_email --is-dev false"
                .cyan()
                .bold()
        )?;

            let raw_url = "https://sandbox.tracer.app";
            let clickable_url = format!("\u{1b}]8;;{0}\u{1b}\\{0}\u{1b}]8;;\u{1b}\\", raw_url);
            let colored_url = clickable_url.cyan().underline().to_string();

            writeln!(
                &mut output,
                "{} {} (see email or visit: {})",
                NEXT,
                "View results in Grafana".bold(),
                colored_url
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

    let current_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    let latest_version = semver::Version::parse(release.tag_name.trim_start_matches('v'))?;

    if latest_version <= current_version {
        println!("\n{} You are already using the latest version of Tracer.", "✓".green());
        println!("Current version: {}.{}.{}", 
            current_version.major,
            current_version.minor,
            current_version.patch
        );
        return Ok(());
    }

    println!("\n{} A new version of Tracer is available!", "↑".yellow());
    println!("Current version: {}.{}.{}", 
        current_version.major,
        current_version.minor,
        current_version.patch
    );
    println!("Latest version:  {}.{}.{}", 
        latest_version.major,
        latest_version.minor,
        latest_version.patch
    );
    println!("\n{} The Tracer daemon will be stopped during the update process.", "⚠️  Warning:".yellow());
    println!("Would you like to proceed with the update? [y/N] ");
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Update cancelled.");
        return Ok(());
    }

    let config = ConfigLoader::load_config(None)?;
    let api_client = DaemonClient::new(format!("http://{}", config.server));

    let _ = api_client.send_terminate_request().await;
    
    // Wait a moment for the daemon to terminate
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    println!("Updating Tracer to version {}.{}.{}", 
        latest_version.major,
        latest_version.minor,
        latest_version.patch
    );

    let mut command = Command::new("bash");
    command.arg("-c").arg(format!("curl -sSL https://install.tracer.cloud | bash -s -- {} && . ~/.bashrc && tracer", config.api_key));

    command
        .status()
        .context("Failed to update Tracer. Please try again.")?;

    let _ = clean_up_after_daemon();

    println!("Update completed successfully. You can now run 'tracer init' to start the daemon with the new version.");
    Ok(())
}
