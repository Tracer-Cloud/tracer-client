use crate::daemon::server::DaemonServer;
use crate::{info_message, warning_message};
use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;
use std::time::Duration;

/// Manages tracer processes during update operations
pub struct ProcessManager {
    pub graceful_timeout: Duration,
    pub force_timeout: Duration,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            graceful_timeout: Duration::from_secs(5),
            force_timeout: Duration::from_secs(2),
        }
    }

    /// Stops all running tracer processes before update
    pub fn stop_tracer_processes(&self) -> Result<()> {
        info_message!("Stopping tracer daemon...");

        if !DaemonServer::is_running() {
            info_message!("No tracer daemon running");
            return Ok(());
        }

        warning_message!("Tracer daemon is running. Attempting to terminate...");

        // Try graceful termination first
        self.try_graceful_termination()?;

        // Give processes time to exit gracefully
        std::thread::sleep(self.graceful_timeout);

        // Force kill if processes are still running
        if self.are_tracer_processes_running()? {
            info_message!("Graceful termination incomplete, trying force kill...");
            self.try_force_termination()?;
            std::thread::sleep(self.force_timeout);
        }

        // Final check
        if self.are_tracer_processes_running()? {
            warning_message!(
                "Some tracer processes may still be running. Update may fail if binary is in use."
            );
        } else {
            info_message!("Tracer processes stopped successfully");
        }

        Ok(())
    }

    fn try_graceful_termination(&self) -> Result<()> {
        let output = Command::new("pkill")
            .args(&["-TERM", "tracer"])
            .output()
            .context("Failed to execute pkill command for graceful termination")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            info_message!("Graceful termination command completed with status: {} (this may be normal if no processes found)", output.status);
            if !stderr.is_empty() {
                info_message!("pkill stderr: {}", stderr);
            }
        }

        Ok(())
    }

    fn try_force_termination(&self) -> Result<()> {
        let output = Command::new("pkill")
            .args(&["-KILL", "tracer"])
            .output()
            .context("Failed to execute pkill command for force termination")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warning_message!("Force termination failed with status: {}", output.status);
            if !stderr.is_empty() {
                warning_message!("pkill stderr: {}", stderr);
            }
        }

        Ok(())
    }

    fn are_tracer_processes_running(&self) -> Result<bool> {
        let output = Command::new("pgrep")
            .args(&["tracer"])
            .output()
            .context("Failed to check for running tracer processes")?;

        Ok(output.status.success() && !output.stdout.is_empty())
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_manager_creation() {
        let pm = ProcessManager::new();
        assert_eq!(pm.graceful_timeout, Duration::from_secs(5));
        assert_eq!(pm.force_timeout, Duration::from_secs(2));
    }

    #[test]
    fn test_process_manager_default() {
        let pm = ProcessManager::default();
        assert_eq!(pm.graceful_timeout, Duration::from_secs(5));
    }

    // Integration tests for actual process management should be in integration tests
    // as they require system-level operations
}
