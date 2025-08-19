use crate::info_message;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Handles atomic binary replacement operations
pub struct BinaryReplacer {
    target_binary_path: String,
    target_directory: String,
    backup_suffix: String,
}

impl BinaryReplacer {
    pub fn new() -> Self {
        Self {
            target_binary_path: "/usr/local/bin/tracer".to_string(),
            target_directory: "/usr/local/bin".to_string(),
            backup_suffix: ".backup".to_string(),
        }
    }

    /// Replaces the tracer binary atomically using rename operation
    pub fn replace_binary_atomically(&self, temp_binary_path: &str) -> Result<()> {
        info_message!("Replacing tracer binary...");

        self.verify_temp_binary_exists(temp_binary_path)?;
        self.verify_write_permissions()?;

        let backup_path = self.create_backup_if_needed()?;

        match self.perform_atomic_replacement(temp_binary_path) {
            Ok(()) => {
                self.set_proper_permissions()?;
                self.cleanup_backup(&backup_path)?;
                info_message!("Binary replacement completed successfully");
                Ok(())
            }
            Err(e) => {
                self.restore_backup_if_exists(&backup_path)?;
                Err(e)
            }
        }
    }



    fn verify_temp_binary_exists(&self, temp_binary: &str) -> Result<()> {
        if !Path::new(temp_binary).exists() {
            return Err(anyhow::anyhow!(
                "Downloaded binary not found at expected location: {}",
                temp_binary
            ));
        }
        Ok(())
    }

    fn verify_write_permissions(&self) -> Result<()> {
        if !self.is_directory_writable(&self.target_directory)? {
            return Err(anyhow::anyhow!(
                "Insufficient permissions to write to {}. Please run with sudo or ensure you have write access.",
                self.target_directory
            ));
        }
        Ok(())
    }

    fn create_backup_if_needed(&self) -> Result<Option<String>> {
        if Path::new(&self.target_binary_path).exists() {
            info_message!("Creating backup of current binary...");
            let backup_path = format!("{}{}", self.target_binary_path, self.backup_suffix);
            fs::copy(&self.target_binary_path, &backup_path)
                .with_context(|| format!("Failed to create backup at {}", backup_path))?;
            Ok(Some(backup_path))
        } else {
            Ok(None)
        }
    }

    fn perform_atomic_replacement(&self, temp_binary: &str) -> Result<()> {
        info_message!("Performing atomic binary replacement...");
        fs::rename(temp_binary, &self.target_binary_path)
            .with_context(|| {
                format!(
                    "Failed to move binary from {} to {}. This is the critical step that replaces the old binary.",
                    temp_binary, self.target_binary_path
                )
            })
    }

    fn set_proper_permissions(&self) -> Result<()> {
        let mut perms = fs::metadata(&self.target_binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&self.target_binary_path, perms)
            .context("Failed to set executable permissions on new binary")
    }

    fn cleanup_backup(&self, backup_path: &Option<String>) -> Result<()> {
        if let Some(backup) = backup_path {
            if Path::new(backup).exists() {
                fs::remove_file(backup).context("Failed to remove backup file")?;
            }
        }
        Ok(())
    }

    fn restore_backup_if_exists(&self, backup_path: &Option<String>) -> Result<()> {
        if let Some(backup) = backup_path {
            if Path::new(backup).exists() {
                info_message!("Restoring backup due to replacement failure...");
                fs::rename(backup, &self.target_binary_path)
                    .context("Failed to restore backup after replacement failure")?;
            }
        }
        Ok(())
    }

    pub fn is_directory_writable(&self, dir_path: &str) -> Result<bool> {
        let test_file = format!("{}/.tracer_update_test", dir_path);

        match fs::File::create(&test_file) {
            Ok(_) => {
                // Clean up test file
                let _ = fs::remove_file(&test_file);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }
}

impl Default for BinaryReplacer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_binary_replacer_creation() {
        let replacer = BinaryReplacer::new();
        assert_eq!(replacer.target_binary_path, "/usr/local/bin/tracer");
        assert_eq!(replacer.target_directory, "/usr/local/bin");
        assert_eq!(replacer.backup_suffix, ".backup");
    }



    #[test]
    fn test_is_directory_writable_with_temp_dir() {
        let replacer = BinaryReplacer::new();
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_str().unwrap();

        // Should be writable in temp directory
        assert!(replacer.is_directory_writable(temp_path).unwrap());
    }

    #[test]
    fn test_is_directory_writable_with_nonexistent_dir() {
        let replacer = BinaryReplacer::new();

        // Should not be writable in non-existent directory
        assert!(!replacer
            .is_directory_writable("/nonexistent/directory")
            .unwrap());
    }

    #[test]
    fn test_binary_replacer_default() {
        let replacer = BinaryReplacer::default();
        assert_eq!(replacer.target_binary_path, "/usr/local/bin/tracer");
    }
}
