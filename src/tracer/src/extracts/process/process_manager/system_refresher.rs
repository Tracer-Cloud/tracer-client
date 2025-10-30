use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::RwLock;
use tracing::debug;

/// Handles system information refresh operations
pub struct SystemRefresher {
    system: Arc<RwLock<System>>,
}

impl SystemRefresher {
    pub fn new() -> Self {
        Self {
            system: Arc::new(RwLock::new(System::new_all())),
        }
    }

    /// Gets a reference to the system
    pub fn get_system(&self) -> &Arc<RwLock<System>> {
        &self.system
    }

    /// Refreshes system information for the specified PIDs
    ///
    /// Uses tokio's spawn_blocking to execute the potentially blocking refresh operation
    /// without affecting the async runtime.
    #[tracing::instrument(skip(self))]
    pub async fn refresh_system(&self, pids: &HashSet<usize>) -> Result<()> {
        // Convert PIDs to the format expected by sysinfo
        let pids_vec = pids.iter().map(|pid| Pid::from(*pid)).collect::<Vec<_>>();

        // Clone the PIDs vector since we need to move it into the spawn_blocking closure
        let pids_for_closure = pids_vec.clone();

        // Get a mutable reference to the system
        let system = Arc::clone(&self.system);

        // Execute the blocking operation in a separate thread
        tokio::task::spawn_blocking(move || {
            let mut sys = system.blocking_write();

            sys.refresh_processes(ProcessesToUpdate::All, true);
        })
        .await?;

        Ok(())
    }
}

impl Default for SystemRefresher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_refresher_creation() {
        let refresher = SystemRefresher::new();

        // Test that we can get a reference to the system
        let system = refresher.get_system();
        let _processes = system.read().await.processes();
        // Just verify we can access the system without panicking
    }

    #[tokio::test]
    async fn test_refresh_system_with_empty_pids() {
        let refresher = SystemRefresher::new();
        let empty_pids = HashSet::new();

        // Should not panic with empty PID set
        let result = refresher.refresh_system(&empty_pids).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_refresh_system_with_current_process() {
        let refresher = SystemRefresher::new();
        let mut pids = HashSet::new();
        pids.insert(std::process::id() as usize); // Current process PID

        // Should successfully refresh current process
        let result = refresher.refresh_system(&pids).await;
        assert!(result.is_ok());
    }
}
