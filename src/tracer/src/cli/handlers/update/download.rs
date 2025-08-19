use crate::info_message;
use crate::utils::env::{self, USER_ID_ENV_VAR};
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::process::Command;

/// Handles downloading new tracer releases
pub struct Downloader {
    base_url: String,
    temp_dir_prefix: String,
}

impl Downloader {
    pub fn new() -> Self {
        Self {
            base_url: "https://install.tracer.cloud".to_string(),
            temp_dir_prefix: "/tmp/tracer-update".to_string(),
        }
    }

    /// Downloads new tracer release to a temporary directory
    /// Returns a tuple of (temp_dir, binary_path)
    pub fn download_new_release(&self) -> Result<(String, String)> {
        info_message!("Downloading new tracer release...");

        let temp_dir = self.create_temp_directory()?;
        self.execute_download(&temp_dir)?;
        let binary_path = self.verify_download(&temp_dir)?;

        info_message!("Download completed successfully");
        Ok((temp_dir, binary_path))
    }

    /// Cleans up temporary directory after update
    pub fn cleanup_temp_dir(&self, temp_dir: &str) -> Result<()> {
        info_message!("Cleaning up temporary files...");

        fs::remove_dir_all(temp_dir)
            .with_context(|| format!("Failed to remove temp directory: {}", temp_dir))?;

        info_message!("Cleanup completed");
        Ok(())
    }

    fn create_temp_directory(&self) -> Result<String> {
        let temp_dir = format!("{}-{}", self.temp_dir_prefix, std::process::id());
        fs::create_dir_all(&temp_dir)
            .with_context(|| format!("Failed to create temp directory: {}", temp_dir))?;
        Ok(temp_dir)
    }

    fn execute_download(&self, temp_dir: &str) -> Result<()> {
        let install_cmd = self.build_install_command(temp_dir);

        let output = Command::new("sh")
            .arg("-c")
            .arg(&install_cmd)
            .output()
            .context("Failed to execute download command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow::anyhow!(
                "Download failed with exit code {}:\nstdout: {}\nstderr: {}",
                output.status.code().unwrap_or(-1),
                stdout,
                stderr
            ));
        }

        Ok(())
    }

    pub fn build_install_command(&self, temp_dir: &str) -> String {
        let user_id_arg = env::get_env_var(USER_ID_ENV_VAR)
            .map(|user_id| {
                let trimmed = user_id.trim();
                if trimmed.is_empty() {
                    "".to_string()
                } else {
                    format!(" -s {}", trimmed)
                }
            })
            .unwrap_or_default();

        // Use a more explicit approach to avoid sudo prompts and ensure correct installation
        format!(
            "cd {} && curl -fsSL {} | INSTALL_DIR={} SKIP_SUDO=1 sh{}",
            temp_dir, self.base_url, temp_dir, user_id_arg
        )
    }

    fn verify_download(&self, temp_dir: &str) -> Result<String> {
        // Try multiple possible locations where the installer might place the binary
        let possible_locations = vec![
            format!("{}/usr/local/bin/tracer", temp_dir),
            format!("{}/tracer", temp_dir),
            format!("{}/bin/tracer", temp_dir),
        ];

        for location in &possible_locations {
            if std::path::Path::new(location).exists() {
                // Verify the binary is executable
                let metadata = fs::metadata(location)
                    .with_context(|| format!("Failed to read metadata for {}", location))?;

                if !metadata.is_file() {
                    continue; // Try next location
                }

                info_message!("Found downloaded binary at: {}", location);
                return Ok(location.clone());
            }
        }

        // If we get here, no binary was found in any expected location
        // List the contents of temp_dir for debugging
        let mut debug_info = format!("Downloaded binary not found in any expected location.\nSearched locations:\n");
        for location in &possible_locations {
            debug_info.push_str(&format!("  - {}\n", location));
        }

        // Add directory contents for debugging
        if let Ok(entries) = fs::read_dir(temp_dir) {
            debug_info.push_str(&format!("\nContents of {}:\n", temp_dir));
            for entry in entries.flatten() {
                debug_info.push_str(&format!("  - {}\n", entry.path().display()));
            }
        }

        Err(anyhow::anyhow!("{}", debug_info))
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downloader_creation() {
        let downloader = Downloader::new();
        assert_eq!(downloader.base_url, "https://install.tracer.cloud");
        assert_eq!(downloader.temp_dir_prefix, "/tmp/tracer-update");
    }

    #[test]
    fn test_build_install_command_without_user_id() {
        let downloader = Downloader::new();
        let temp_dir = "/tmp/test";
        let cmd = downloader.build_install_command(temp_dir);

        assert!(cmd.contains("cd /tmp/test"));
        assert!(cmd.contains("curl -fsSL https://install.tracer.cloud"));
        assert!(cmd.contains("INSTALL_DIR=/tmp/test"));
    }

    #[test]
    fn test_build_install_command_with_user_id() {
        std::env::set_var(USER_ID_ENV_VAR, "test-user");

        let downloader = Downloader::new();
        let temp_dir = "/tmp/test";
        let cmd = downloader.build_install_command(temp_dir);

        assert!(cmd.contains(" -s test-user"));

        std::env::remove_var(USER_ID_ENV_VAR);
    }

    #[test]
    fn test_downloader_default() {
        let downloader = Downloader::default();
        assert_eq!(downloader.base_url, "https://install.tracer.cloud");
    }
}
