use crate::sentry::Sentry;
use crate::utils::print_title;
use checks::CheckManager;
use clap::Parser;
use installer::{Installer, PlatformInfo};
use serde_json::json;
use types::{InstallTracerCli, InstallerCommand};
use utils::print_anteater_banner;

mod checks;
mod constants;
mod fs;
mod installer;
mod message;
mod sentry;
mod types;
mod utils;

use crate::sentry::Sentry;
use crate::utils::print_title;
use checks::CheckManager;
use clap::Parser;
use installer::{Installer, PlatformInfo};
use tokio::runtime::Runtime;
use types::{InstallTracerCli, InstallerCommand};
use utils::print_anteater_banner;

fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let _guard = Sentry::setup();

    // Wrap main logic in Sentry error handling
    if let Err(e) = run_installer().await {
        capture_installer_error(&e);
        eprintln!("Error Running Installer: {e}");
        std::process::exit(1);
    }
}

async fn run_installer() -> anyhow::Result<()> {
    let args = InstallTracerCli::parse();

    let runtime = Runtime::new().expect("Failed to create tokio runtime");

    runtime.block_on(async_main(args));
}

async fn async_main(args: InstallTracerCli) {
    match args.command {
        InstallerCommand::Run { channel, user_id } => {
            // Add initial installer context
            Sentry::add_tag("installer_channel", &channel.to_string());
            if let Some(ref _uid) = user_id {
                Sentry::add_tag("has_user_id", "true");
                // Don't log the actual user_id for privacy
            } else {
                Sentry::add_tag("has_user_id", "false");
            }

            // Run checks
            print_anteater_banner(&channel);

            print_title("System Specification");

            // Platform detection
            Sentry::add_tag("installer_stage", "platform_detection");
            let platform = PlatformInfo::build()
                .map_err(|e| {
                    Sentry::add_tag("error_stage", "platform_detection");
                    anyhow::anyhow!("Failed to detect platform: {e}")
                })?;

            platform.print_summary();

            print_title("Running Environment Checks");

            // Environment checks
            Sentry::add_tag("installer_stage", "environment_checks");
            let requirements = CheckManager::new(&platform).await;
            requirements.run_all().await;

            print_title("Installing Tracer");

            // Binary installation
            Sentry::add_tag("installer_stage", "binary_installation");
            let installer = Installer {
                platform: platform.clone(),
                channel: channel.clone(),
                user_id: user_id.clone(),
            };

            // Add installer context before running
            Sentry::add_context("installer_config", json!({
                "platform_os": format!("{:?}", platform.os),
                "platform_arch": format!("{:?}", platform.arch),
                "platform_full_os": platform.full_os,
                "channel": channel.to_string(),
            }));

            installer.run().await.map_err(|e| {
                Sentry::add_tag("error_stage", "binary_installation");
                e
            })?;

            // Mark successful completion
            Sentry::add_tag("installer_stage", "completed");
            Sentry::capture_message("Installer completed successfully", ::sentry::Level::Info);
        }
    }

    Ok(())
}

/// Captures comprehensive installer error information to Sentry
fn capture_installer_error(error: &anyhow::Error) {
    // Add installer-specific tags
    Sentry::add_tag("installer_stage", "main_execution");
    Sentry::add_tag("error_source", "installer");

    // Add detailed error context
    let error_chain: Vec<String> = error.chain().map(|e| e.to_string()).collect();
    let root_cause = error.root_cause().to_string();

    Sentry::add_context("error_details", json!({
        "error_message": error.to_string(),
        "root_cause": root_cause,
        "error_chain_length": error_chain.len(),
        "error_type": std::any::type_name::<anyhow::Error>(),
    }));

    // Add full error chain as extra data for detailed debugging
    Sentry::add_extra("error_chain", json!(error_chain));

    // Add system information at time of error
    let system_info = get_error_system_info();
    Sentry::add_context("system_state", system_info);

    // Capture the error message with high priority
    Sentry::capture_message(
        &format!("Installer failed: {}", error),
        ::sentry::Level::Error
    );
}

/// Collects system information relevant to installer errors
fn get_error_system_info() -> serde_json::Value {
    json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "family": std::env::consts::FAMILY,
        "current_dir": std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        "temp_dir": std::env::temp_dir().to_string_lossy().to_string(),
        "user": std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
        "home": std::env::var("HOME").unwrap_or_else(|_| "unknown".to_string()),
        "path_exists_usr_local_bin": std::path::Path::new("/usr/local/bin").exists(),
        "path_writable_usr_local_bin": is_writable("/usr/local/bin"),
    })
}

/// Checks if a directory is writable
fn is_writable(path: &str) -> bool {
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(format!("{}/tracer_write_test", path))
        .and_then(|_| std::fs::remove_file(format!("{}/tracer_write_test", path)))
        .is_ok()
}
