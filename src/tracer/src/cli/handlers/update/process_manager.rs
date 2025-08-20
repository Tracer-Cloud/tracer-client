use crate::daemon::server::DaemonServer;
use crate::{info_message, warning_message};
use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;
use std::time::Duration;

/// Manages tracer processes during update operations
#[allow(dead_code)] // Suppress warnings for unused fields
pub struct ProcessManager {
    pub graceful_timeout: Duration,
    pub force_timeout: Duration,
}

#[allow(dead_code)] // Suppress warnings for unused methods
impl ProcessManager {
    pub fn new() -> Self {
        Self {
            graceful_timeout: Duration::from_secs(5),
            force_timeout: Duration::from_secs(2),
        }
    }

    /// Stops all running tracer processes before update
    pub fn stop_tracer_processes(&self) -> Result<()> {
        info_message!("🔍 Checking for running tracer processes...");

        if !DaemonServer::is_running() {
            info_message!("✅ No tracer daemon running - proceeding with update");
            return Ok(());
        }

        info_message!("🛑 Tracer daemon detected - initiating graceful shutdown...");

        // Try HTTP API termination first (more graceful)
        if self.try_http_termination().is_ok() {
            info_message!("✅ Daemon terminated via HTTP API - waiting for cleanup...");
            std::thread::sleep(self.graceful_timeout);
        }

        // If still running, try graceful termination with signals
        if self.are_tracer_processes_running()? {
            info_message!("🔄 Using signal-based termination for remaining processes...");
            self.try_graceful_termination()?;
            std::thread::sleep(self.graceful_timeout);
        }

        // Force kill if processes are still running
        if self.are_tracer_processes_running()? {
            info_message!("⚡ Some processes need force termination...");
            self.try_force_termination()?;
            std::thread::sleep(self.force_timeout);
        }

        // Final check with extended wait
        if self.are_tracer_processes_running()? {
            info_message!("⏳ Waiting for final process cleanup...");
            std::thread::sleep(Duration::from_secs(3));

            if self.are_tracer_processes_running()? {
                warning_message!(
                    "⚠️  Some tracer processes may still be running. Update will continue but may fail if binary is in use."
                );
            } else {
                info_message!("✅ All tracer processes stopped successfully");
            }
        } else {
            info_message!("✅ All tracer processes stopped successfully");
        }

        Ok(())
    }

    fn try_http_termination(&self) -> Result<()> {
        use crate::daemon::client::DaemonClient;
        use crate::process_identification::constants::DEFAULT_DAEMON_PORT;

        let api_client = DaemonClient::new(format!("http://127.0.0.1:{}", DEFAULT_DAEMON_PORT));

        // Use async runtime to call the terminate API
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async { api_client.send_terminate_request().await })?;

        Ok(())
    }

    fn try_graceful_termination(&self) -> Result<()> {
        // Get current process PID to avoid killing ourselves
        let current_pid = std::process::id();

        // Use pgrep to find tracer processes, then filter out current process
        let pgrep_output = Command::new("pgrep")
            .arg("tracer")
            .output()
            .context("Failed to execute pgrep command")?;

        if pgrep_output.status.success() {
            let pids_str = String::from_utf8_lossy(&pgrep_output.stdout);
            let target_pids: Vec<u32> = pids_str
                .lines()
                .filter_map(|line| line.trim().parse::<u32>().ok())
                .filter(|&pid| pid != current_pid) // Don't kill ourselves!
                .collect();

            if target_pids.is_empty() {
                info_message!("No other tracer processes found to terminate");
                return Ok(());
            }

            info_message!(
                "Sending SIGTERM to {} tracer process(es) (excluding current update process)",
                target_pids.len()
            );

            // Send SIGTERM to each target process individually
            for pid in target_pids {
                let _ = Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .output();
            }
        } else {
            info_message!("No tracer processes found to terminate");
        }

        Ok(())
    }

    fn try_force_termination(&self) -> Result<()> {
        // Get current process PID to avoid killing ourselves
        let current_pid = std::process::id();

        // Use pgrep to find tracer processes, then filter out current process
        let pgrep_output = Command::new("pgrep")
            .arg("tracer")
            .output()
            .context("Failed to execute pgrep command")?;

        if pgrep_output.status.success() {
            let pids_str = String::from_utf8_lossy(&pgrep_output.stdout);
            let target_pids: Vec<u32> = pids_str
                .lines()
                .filter_map(|line| line.trim().parse::<u32>().ok())
                .filter(|&pid| pid != current_pid) // Don't kill ourselves!
                .collect();

            if target_pids.is_empty() {
                info_message!("No other tracer processes found for force termination");
                return Ok(());
            }

            warning_message!(
                "Force killing {} stubborn tracer process(es) (excluding current update process)",
                target_pids.len()
            );

            // Send SIGKILL to each target process individually
            for pid in target_pids {
                let _ = Command::new("kill")
                    .args(["-KILL", &pid.to_string()])
                    .output();
            }
        } else {
            info_message!("No tracer processes found for force termination");
        }

        Ok(())
    }

    fn are_tracer_processes_running(&self) -> Result<bool> {
        let output = Command::new("pgrep")
            .args(["tracer"])
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
