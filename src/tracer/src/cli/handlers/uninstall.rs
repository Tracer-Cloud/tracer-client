use crate::daemon::server::DaemonServer;
use crate::process_identification::constants::WORKING_DIR;
use crate::utils::system_info::check_sudo;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

const INSTALL_PATH: &str = "/usr/local/bin/tracer";

pub fn uninstall() -> Result<()> {
    if DaemonServer::is_running() {
        println!("\n{} Tracer daemon is currently running. Please run `tracer terminate` before uninstalling", "Warning:".yellow());
        return Ok(());
    }

    check_sudo("uninstall");

    println!(">> Uninstalling Tracer...");

    if Path::new(WORKING_DIR).exists() {
        fs::remove_dir_all(WORKING_DIR)?;
        println!(
            "✅  Working directory removed successfully: {}",
            WORKING_DIR
        );
    } else {
        println!("Working directory {} does not exist", WORKING_DIR);
    }
    println!();
    remove_binary()?;
    println!();
    remove_env_paths()?;
    println!("✅  Tracer uninstalled successfully");

    Ok(())
}

fn remove_binary() -> Result<()> {
    let tracer_path = Path::new(INSTALL_PATH);

    if tracer_path.exists() {
        println!("🔍 Binary path: {}", tracer_path.display());
        fs::remove_file(tracer_path)
            .with_context(|| format!("Failed to remove binary at {}", tracer_path.display()))?;
        println!("✅  Binary removed successfully");
    } else {
        println!("⚠️  Binary not found at: {}", tracer_path.display());
    }

    Ok(())
}
fn remove_env_paths() -> Result<()> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;

    let profile_files = [".bashrc", ".bash_profile", ".zshrc", ".profile"];
    for profile in &profile_files {
        let profile_path = home_dir.join(profile);

        if profile_path.exists() {
            remove_env(&profile_path)?;
        }
    }

    println!("✅  Tracer environment variables removed (if any)");
    Ok(())
}

fn remove_env(file_path: &Path) -> Result<()> {
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;

    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::new();
    let mut removed_lines = Vec::new();
    let mut in_tracer_block = false;

    for line in lines {
        let trimmed = line.trim();

        // Check for Tracer-related content
        if trimmed.to_lowercase().contains("tracer") || trimmed.contains("TRACER_USER_ID") {
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
        println!(
            "Removing Tracer environment variables from: {}",
            file_path.display()
        );
        for line in &removed_lines {
            println!("  - {}", line.trim());
        }

        let new_content = new_lines.join("\n");
        fs::write(file_path, new_content)
            .with_context(|| format!("Failed to write updated {}", file_path.display()))?;

        println!("✅  Updated {}", file_path.display());
        println!();
    }

    Ok(())
}
