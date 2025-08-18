use crate::extracts::process::process_manager::recorder::EventRecorder;
use crate::extracts::process::process_manager::state::StateManager;
use crate::extracts::process::process_manager::system_refresher::SystemRefresher;
use anyhow::Result;
use std::collections::HashSet;
use sysinfo::System;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::{debug, warn};

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
        event_recorder: &EventRecorder,
        system_refresher: &SystemRefresher,
    ) -> Result<()> {
        debug!("Starting periodic process metrics polling");

        // Get all monitored process PIDs
        let monitored_pids = state_manager.get_monitored_processes_pids().await;

        if monitored_pids.is_empty() {
            warn!("No processes are currently monitored - skipping metrics poll");
            return Ok(());
        }

        // Refresh system data for all monitored processes
        system_refresher.refresh_system(&monitored_pids).await?;
        debug!("System data refreshed for {} PIDs", monitored_pids.len());

        let system = system_refresher.get_system().read().await; // Acquire the lock once

        // Extract and log metrics for each monitored process
        for (target, processes) in state_manager.get_state().await.get_monitoring().iter() {
            process_metrics_for_target(target, processes, &system, logger).await?;
        }

        Ok(())
    }
}

// Extract and log metrics for a single target
pub async fn process_metrics_for_target(
    target: &String,
    processes: &HashSet<ProcessStartTrigger>,
    system: &System,
    logger: &ProcessLogger,
) -> Result<()> {
    for process in processes {
        if let Some(process_data) = system.process(process.pid.into()) {
            let result = event_recorder
                .log_process_metrics(target, process, Some(process_data))
                .await?;
            debug!("Metrics extracted for PID {}: {:?}", process.pid, result);
        }
    }

    Ok(())
}
