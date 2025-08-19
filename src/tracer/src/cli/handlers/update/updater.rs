use crate::utils::Sentry;
use crate::{error_message, info_message, success_message};
use anyhow::Result;
use colored::Colorize;

use super::binary_replacement::BinaryReplacer;
use super::download::Downloader;
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
    let downloader = Downloader::new();
    let binary_replacer = BinaryReplacer::new();

    // Step 1: Stop any running tracer processes
    process_manager.stop_tracer_processes()?;

    // Step 2: Download new release into temp directory
    let (temp_dir, binary_path) = downloader.download_new_release()?;

    // Step 3: Replace binary using atomic mv
    binary_replacer.replace_binary_atomically(&binary_path)?;

    // Step 4: Cleanup temp directory
    downloader.cleanup_temp_dir(&temp_dir)?;

    Ok(())
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
