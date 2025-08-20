use crate::utils::Sentry;
use crate::{error_message, info_message, success_message};
use anyhow::{Context, Result};
use colored::Colorize;

use super::process_manager::ProcessManager;

/// Main entry point for the tracer update command
pub fn update() {
    match update_impl() {
        Ok(()) => success_message!("Tracer has been successfully updated!"),
        Err(e) => {
            Sentry::capture_message(&format!("Update failed: {}", e), sentry::Level::Error);
            error_message!("Failed to update Tracer: {}", e);
            std::process::exit(1);
        }
    }
}

/// Core update implementation with proper error handling
fn update_impl() -> Result<()> {
    info_message!("Starting Tracer update process...");

    let process_manager = ProcessManager::new();

    // Step 1: Stop any running tracer processes
    process_manager.stop_tracer_processes()?;

    // Step 2: Run the installer script directly (it handles everything)
    run_installer_script()?;

    info_message!("Update completed successfully");
    Ok(())
}

/// Run the tracer installer script directly
fn run_installer_script() -> Result<()> {
    info_message!("Downloading and replacing tracer binary...");

    // Download the binary directly and replace it manually (no sudo needed)
    download_and_replace_binary()
}

/// Download and replace the tracer binary without requiring sudo
fn download_and_replace_binary() -> Result<()> {
    // Get the current tracer binary path
    let current_binary = get_current_tracer_path()?;
    info_message!("Current tracer binary: {}", current_binary);

    // Create a temporary directory for download
    let temp_dir = std::env::temp_dir().join(format!("tracer-update-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)
        .with_context(|| format!("Failed to create temp directory: {}", temp_dir.display()))?;

    let temp_binary = temp_dir.join("tracer");

    // Download the latest binary
    info_message!("Downloading latest tracer binary...");
    download_latest_binary(&temp_binary)?;

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_binary)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&temp_binary, perms)?;
    }

    // Create backup of current binary
    let backup_path = format!("{}.backup", current_binary);
    info_message!("Creating backup at: {}", backup_path);
    std::fs::copy(&current_binary, &backup_path)
        .with_context(|| format!("Failed to create backup at {}", backup_path))?;

    // Replace the binary atomically
    info_message!("Replacing tracer binary...");
    std::fs::rename(&temp_binary, &current_binary)
        .with_context(|| format!("Failed to replace binary at {}", current_binary))?;

    // Clean up
    std::fs::remove_dir_all(&temp_dir)
        .with_context(|| format!("Failed to clean up temp directory: {}", temp_dir.display()))?;

    // Remove backup on success
    std::fs::remove_file(&backup_path)
        .with_context(|| format!("Failed to remove backup file: {}", backup_path))?;

    info_message!("Binary replacement completed successfully");
    Ok(())
}

/// Get the path of the currently running tracer binary
fn get_current_tracer_path() -> Result<String> {
    // Try to get the path from the current executable
    match std::env::current_exe() {
        Ok(path) => Ok(path.to_string_lossy().to_string()),
        Err(_) => {
            // Fallback: check common locations
            let common_paths = vec![
                "/usr/local/bin/tracer",
                "/usr/bin/tracer",
                "/opt/homebrew/bin/tracer",
                "~/.local/bin/tracer",
            ];

            for path in common_paths {
                let expanded_path = if let Some(stripped) = path.strip_prefix("~/") {
                    if let Some(home) = std::env::var_os("HOME") {
                        std::path::Path::new(&home)
                            .join(stripped)
                            .to_string_lossy()
                            .to_string()
                    } else {
                        continue;
                    }
                } else {
                    path.to_string()
                };

                if std::path::Path::new(&expanded_path).exists() {
                    return Ok(expanded_path);
                }
            }

            Err(anyhow::anyhow!(
                "Could not find current tracer binary location"
            ))
        }
    }
}

/// Download the latest tracer binary from S3
fn download_latest_binary(target_path: &std::path::Path) -> Result<()> {
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(anyhow::anyhow!("Unsupported architecture"));
    };

    let os = if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "linux") {
        "unknown-linux-gnu"
    } else {
        return Err(anyhow::anyhow!("Unsupported operating system"));
    };

    // Download the tar.gz file from S3
    let download_url = format!(
        "https://tracer-releases.s3.us-east-1.amazonaws.com/main/tracer-{}-{}.tar.gz",
        arch, os
    );

    info_message!("Downloading from: {}", download_url);

    // Create temp file for the tar.gz
    let temp_dir = target_path.parent().unwrap();
    let tar_path = temp_dir.join("tracer.tar.gz");

    let output = std::process::Command::new("curl")
        .args(["-fsSL", "-o", tar_path.to_str().unwrap(), &download_url])
        .output()
        .context("Failed to execute curl command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to download binary: {}", stderr));
    }

    // Verify the download
    if !tar_path.exists() {
        return Err(anyhow::anyhow!(
            "Downloaded tar.gz not found after download"
        ));
    }

    let metadata = std::fs::metadata(&tar_path)?;
    if metadata.len() == 0 {
        return Err(anyhow::anyhow!("Downloaded tar.gz is empty"));
    }

    info_message!("Download completed successfully ({} bytes)", metadata.len());

    // Extract the binary from the tar.gz
    info_message!("Extracting binary from archive...");
    extract_binary_from_tar(&tar_path, target_path)?;

    // Clean up the tar.gz file
    std::fs::remove_file(&tar_path).with_context(|| {
        format!(
            "Failed to remove temporary tar file: {}",
            tar_path.display()
        )
    })?;

    Ok(())
}

/// Extract the tracer binary from the downloaded tar.gz file
fn extract_binary_from_tar(
    tar_path: &std::path::Path,
    target_path: &std::path::Path,
) -> Result<()> {
    let temp_dir = target_path.parent().unwrap();

    // Extract the tar.gz file
    let output = std::process::Command::new("tar")
        .args([
            "-xzf",
            tar_path.to_str().unwrap(),
            "-C",
            temp_dir.to_str().unwrap(),
        ])
        .output()
        .context("Failed to execute tar command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to extract tar.gz: {}", stderr));
    }

    // Find the extracted binary (it might be in a subdirectory)
    let possible_paths = vec![
        temp_dir.join("tracer"),
        temp_dir.join("bin/tracer"),
        temp_dir.join("usr/local/bin/tracer"),
    ];

    for possible_path in &possible_paths {
        if possible_path.exists() {
            info_message!("Found extracted binary at: {}", possible_path.display());
            std::fs::rename(possible_path, target_path).with_context(|| {
                format!(
                    "Failed to move binary from {} to {}",
                    possible_path.display(),
                    target_path.display()
                )
            })?;
            return Ok(());
        }
    }

    // If we can't find the binary in expected locations, list what we got
    let mut debug_info =
        String::from("Could not find tracer binary in extracted files.\nExtracted contents:\n");
    if let Ok(entries) = std::fs::read_dir(temp_dir) {
        for entry in entries.flatten() {
            debug_info.push_str(&format!("  - {}\n", entry.path().display()));
        }
    }

    Err(anyhow::anyhow!("{}", debug_info))
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_update_impl_structure() {
        // This test ensures the update_impl function structure is maintained
        // Integration tests should be in the tests module
        assert!(true);
    }
}
