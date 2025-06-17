use crate::extracts::process::process_manager::logger::ProcessLogger;
use crate::extracts::process::process_manager::matcher::Filter;
use crate::extracts::process::process_manager::state::StateManager;
use crate::extracts::process::process_manager::system_refresher::SystemRefresher;
use crate::extracts::process::types::process_result::ProcessResult;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::debug;

/// Handles process start events
pub struct ProcessStartHandler;

impl ProcessStartHandler {
    /// Handles newly started processes by filtering, gathering data, and setting up monitoring
    ///
    /// This function:
    /// 1. Filters processes to find those matching our target criteria
    /// 2. Refreshes system data for matched processes
    /// 3. Extracts and logs data for each process
    /// 4. Updates the monitoring state to track these processes
    pub async fn handle_process_starts(
        state_manager: &StateManager,
        logger: &ProcessLogger,
        matcher: &Filter,
        system_refresher: &SystemRefresher,
        triggers: Vec<ProcessStartTrigger>,
    ) -> Result<()> {
        let trigger_count = triggers.len();
        debug!("Processing {} process starts", trigger_count);

        // Find processes we're interested in based on targets
        let filtered_target_processes = {
            // Store all triggers in the state first
            for trigger in triggers.iter() {
                state_manager.insert_process(trigger.pid, trigger.clone()).await;
            }
            
            let state = state_manager.get_state().await;
            matcher.filter_processes_of_interest(triggers, &state).await?
        };
        
        let matched_count = filtered_target_processes.len();
        debug!("After filtering, matched {} processes out of {}", matched_count, trigger_count);

        if filtered_target_processes.is_empty() {
            return Ok(());
        }

        // Collect all PIDs that need system data refreshed
        let pids_to_refresh = matcher.collect_pids_to_refresh(&filtered_target_processes);
        
        // Refresh system data for these processes
        system_refresher.refresh_system(&pids_to_refresh).await?;

        // Process each matched process
        Self::process_matched_processes(logger, system_refresher, &filtered_target_processes).await?;

        // Update monitoring state with new processes
        state_manager.update_monitoring(filtered_target_processes).await?;

        Ok(())
    }

    /// Processes each matched process by extracting and logging its data
    async fn process_matched_processes(
        logger: &ProcessLogger,
        system_refresher: &SystemRefresher,
        filtered_target_processes: &HashMap<crate::common::target_process::Target, HashSet<ProcessStartTrigger>>,
    ) -> Result<()> {
        for (target, processes) in filtered_target_processes.iter() {
            for process in processes.iter() {
                let system = system_refresher.get_system().read().await;
                let system_process = system.process(process.pid.into());
                
                logger.log_new_process(target, process, system_process).await?;
            }
        }
        Ok(())
    }

    /// Updates all monitored processes with fresh data
    pub async fn update_all_processes(
        state_manager: &StateManager,
        logger: &ProcessLogger,
        system_refresher: &SystemRefresher,
    ) -> Result<()> {
        for (target, procs) in state_manager.get_state().await.get_monitoring().iter() {
            for proc in procs.iter() {
                let system = system_refresher.get_system().read().await;
                let system_process = system.process(proc.pid.into());

                let result = logger.log_process_metrics(target, proc, system_process).await?;

                match result {
                    ProcessResult::NotFound => {
                        // TODO: Mark process as completed
                        debug!("Process {} was not found during update", proc.pid);
                    }
                    ProcessResult::Found => {}
                }
            }
        }

        Ok(())
    }

    /// Polls and updates metrics for all monitored processes
    pub async fn poll_process_metrics(
        state_manager: &StateManager,
        logger: &ProcessLogger,
        system_refresher: &SystemRefresher,
    ) -> Result<()> {
        debug!("Polling process metrics");

        // Get PIDs of all monitored processes
        let pids = state_manager.get_monitored_processes_pids().await;
        
        debug!("Refreshing data for {} processes", pids.len());

        if pids.is_empty() {
            debug!("No processes to monitor, skipping poll");
            return Ok(());
        }

        // Refresh system data and process updates
        system_refresher.refresh_system(&pids).await?;
        Self::update_all_processes(state_manager, logger, system_refresher).await?;

        debug!("Refreshing data completed");

        Ok(())
    }
}
