use crate::opentelemetry::collector::OtelCollector;
use crate::{error_message, info_message, success_message, warning_message};
use anyhow::Result;
use colored::Colorize;
use std::env;
use std::process::Command;

pub async fn setup() -> Result<()> {
    // Run the setup in a blocking task to avoid runtime conflicts
    tokio::task::spawn_blocking(|| setup_sync()).await?
}

pub fn setup_sync() -> Result<()> {
    info_message!("Setting up OpenTelemetry collector...");

    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    info_message!("Detected platform: {} on {}", os, arch);

    // Check if we're on a supported platform
    let (platform, arch_name) = match (os, arch) {
        ("linux", "x86_64") => ("linux", "amd64"),
        ("linux", "aarch64") => ("linux", "arm64"),
        ("macos", "x86_64") => ("darwin", "amd64"),
        ("macos", "aarch64") => ("darwin", "arm64"),
        _ => {
            error_message!("Unsupported platform: {} on {}", os, arch);
            error_message!("Supported platforms: Linux (x86_64, aarch64), macOS (x86_64, aarch64)");
            return Err(anyhow::anyhow!("Unsupported platform: {} on {}", os, arch));
        }
    };

    info_message!("Platform mapping: {} -> {}", os, platform);
    info_message!("Architecture mapping: {} -> {}", arch, arch_name);

    // Create the collector instance
    let collector = OtelCollector::new()?;

    // Check if already installed
    if collector.is_installed() {
        if let Some(version) = collector.get_version() {
            info_message!(
                "OpenTelemetry collector is already installed (version: {})",
                version
            );
        } else {
            info_message!("OpenTelemetry collector is already installed");
        }

        // Verify the installation works
        info_message!("Verifying installation...");
        if let Ok(output) = Command::new(collector.binary_path())
            .arg("--version")
            .output()
        {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stdout);
                success_message!("OpenTelemetry collector is working correctly");
                info_message!("Version output: {}", version_output.trim());
                return Ok(());
            }
        }

        warning_message!("Existing installation appears to be broken, reinstalling...");
    }

    // Install the collector
    info_message!("Installing OpenTelemetry collector...");
    collector.install()?;

    // Verify the installation
    info_message!("Verifying installation...");
    if let Ok(output) = Command::new(collector.binary_path())
        .arg("--version")
        .output()
    {
        if output.status.success() {
            let version_output = String::from_utf8_lossy(&output.stdout);
            success_message!("OpenTelemetry collector installed successfully!");
            info_message!("Version: {}", version_output.trim());
            info_message!("Binary location: {:?}", collector.binary_path());

            // Test basic functionality
            if let Ok(help_output) = Command::new(collector.binary_path()).arg("--help").output() {
                if help_output.status.success() {
                    info_message!("Basic functionality test passed");
                } else {
                    warning_message!("Basic functionality test failed");
                }
            }

            return Ok(());
        }
    }

    error_message!("Installation verification failed");
    Err(anyhow::anyhow!(
        "Failed to verify OpenTelemetry collector installation"
    ))
}
