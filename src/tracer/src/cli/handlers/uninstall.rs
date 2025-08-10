use crate::daemon::server::DaemonServer;
use crate::utils::env::USER_ID_ENV_VAR;
use crate::utils::secure::{TrustedDir, TrustedFile};
use crate::utils::system_info::check_sudo;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::{success_message, warning_message};
use anyhow::{Context, Result};
use colored::Colorize;
use std::sync::LazyLock;

static INSTALL_PATH: LazyLock<TrustedFile> = LazyLock::new(|| {
    TrustedFile::tracer_binary().expect("could not obtain trusted path to tracer binary")
});

pub fn uninstall() {
    if DaemonServer::is_running() {
        warning_message!(
            "Tracer daemon is currently running. Please run `tracer terminate` before uninstalling"
        );
        return;
    }

    check_sudo("uninstall");

    println!(">> Uninstalling Tracer...");

    TRACER_WORK_DIR.cleanup().unwrap();
    println!();
    remove_binary().unwrap();
    println!();
    remove_env_paths().unwrap();
    success_message!("Tracer uninstalled successfully");
}

fn remove_binary() -> Result<()> {
    let tracer_binary = &INSTALL_PATH;

    if tracer_binary.exists()? {
        println!("🔍 Binary path: {}", tracer_binary.to_string());
        tracer_binary
            .remove()
            .with_context(|| format!("Failed to remove binary at {}", tracer_binary.to_string()))?;
        success_message!("Binary removed successfully");
    } else {
        warning_message!("Binary not found at: {}", tracer_binary.to_string());
    }

    Ok(())
}

fn remove_env_paths() -> Result<()> {
    let home_dir = TrustedDir::home()?;

    let profile_files = [".bashrc", ".bash_profile", ".zshrc", ".profile"];
    for profile in &profile_files {
        let profile_path = home_dir.get_trusted_file((*profile).try_into()?)?;
        if profile_path.exists()? {
            remove_env(&profile_path)?;
        }
    }

    success_message!("Tracer environment variables removed (if any)");
    Ok(())
}

fn remove_env(file_path: &TrustedFile) -> Result<()> {
    let content = file_path.read_to_string()?;

    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::new();
    let mut removed_lines = Vec::new();
    let mut in_tracer_block = false;

    for line in lines {
        let trimmed = line.trim();

        // Check for Tracer-related content
        if trimmed.to_lowercase().contains("tracer") || trimmed.contains(USER_ID_ENV_VAR) {
            removed_lines.push(line);
            in_tracer_block = true;
            continue;
        }

        // Skip empty lines that follow tracer content
        if in_tracer_block && trimmed.is_empty() {
            continue;
        }

        in_tracer_block = false;
        new_lines.push(line);
    }

    if !removed_lines.is_empty() {
        println!("Removing Tracer environment variables from: {}", file_path);
        for line in &removed_lines {
            println!("  - {}", line.trim());
        }

        let new_content = new_lines.join("\n");
        file_path.write(&new_content)?;

        success_message!("Updated {}", file_path);
        println!();
    }

    Ok(())
}
