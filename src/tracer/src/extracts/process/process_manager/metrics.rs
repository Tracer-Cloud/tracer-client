use crate::extracts::process::process_manager::logger::ProcessLogger;
use crate::extracts::process::process_manager::state::StateManager;
use crate::extracts::process::process_manager::system_refresher::SystemRefresher;
use crate::extracts::process::types::process_result::ProcessResult;
use anyhow::Result;
use tracing::debug;

/// Handles periodic polling and updating of process metrics for monitored processes.
///
/// This handler is responsible for:
/// - Periodically refreshing system data for all monitored processes
/// - Extracting and logging updated metrics for each process
/// - Detecting processes that are no longer running
///
/// This is separate from event-driven process handling and runs on a periodic schedule.
pub struct ProcessMetricsHandler;

impl ProcessMetricsHandler {
    /// Polls and updates metrics for all monitored processes.
    ///
    /// This method:
    /// 1. Gets the list of all currently monitored process PIDs
    /// 2. Refreshes system data for those processes
    /// 3. Iterates through all monitored processes and logs updated metrics
    /// 4. Detects and logs processes that are no longer running
    ///
    /// This is typically called on a periodic schedule (e.g., every few seconds)
    /// to keep process metrics up to date.
    pub async fn poll_process_metrics(
        state_manager: &StateManager,
        logger: &ProcessLogger,
        system_refresher: &SystemRefresher,
    ) -> Result<()> {
        debug!("Starting periodic process metrics polling");

        // Step 1: Get all monitored process PIDs
        let monitored_pids = state_manager.get_monitored_processes_pids().await;

        if monitored_pids.is_empty() {
            debug!("No processes are currently monitored - skipping metrics poll");
            return Ok(());
        }

        debug!(
            "Polling metrics for {} monitored processes",
            monitored_pids.len()
        );

        // Step 2: Refresh system data for all monitored processes
        system_refresher.refresh_system(&monitored_pids).await?;
        debug!("System data refreshed for {} PIDs", monitored_pids.len());

        // Step 3: Extract and log metrics for each monitored process
        let mut processed_count = 0;
        let mut not_found_count = 0;

        for (target, processes) in state_manager.get_state().await.get_monitoring().iter() {
            for proc in processes {
                let system = system_refresher.get_system().read().await;
                let sys_proc = system.process(proc.pid.into());

                let result = logger.log_process_metrics(target, proc, sys_proc).await?;

                match result {
                    ProcessResult::Found => {
                        processed_count += 1;
                    }
                    ProcessResult::NotFound => {
                        debug!("Process PID={} no longer running - will be cleaned up on next termination event", proc.pid);
                        not_found_count += 1;

                        // TODO bug process polling on EXITTRIGGER
                    }
                }
            }
        }

        debug!(
            "Metrics polling completed: {} processes updated, {} not found",
            processed_count, not_found_count
        );

        Ok(())
    }
}
