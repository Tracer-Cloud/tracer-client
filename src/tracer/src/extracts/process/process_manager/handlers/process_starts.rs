use crate::extracts::process::process_manager::logger::ProcessLogger;
use crate::extracts::process::process_manager::matcher::Filter;
use crate::extracts::process::process_manager::state::StateManager;
use crate::extracts::process::process_manager::system_refresher::SystemRefresher;
use anyhow::Result;
use std::collections::HashSet;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::debug;

/// Handles process start events.
pub struct ProcessStartHandler;

impl ProcessStartHandler {
    /// Handles newly started processes through simplified linear steps.
    pub async fn handle_process_starts(
        state_manager: &StateManager,
        logger: &ProcessLogger,
        matcher: &Filter,
        system_refresher: &SystemRefresher,
        triggers: Vec<ProcessStartTrigger>,
    ) -> Result<()> {
        debug!("Handling {} process starts", triggers.len());

        // Step 1: Store triggers for relationship tracking
        for trigger in &triggers {
            state_manager.insert_process(trigger.pid, trigger.clone()).await;
        }

        // Step 2: Match triggers against targets
        let matched_processes = {
            let state = state_manager.get_state().await;
            matcher.filter_processes_of_interest(triggers, &state).await?
        };

        if matched_processes.is_empty() {
            debug!("No processes matched; pipeline completed early.");
            return Ok(());
        }

        // Step 3: Refresh system data
        let pids_to_refresh: HashSet<_> = matched_processes
            .values()
            .flatten()
            .map(|p| p.pid)
            .collect();

        system_refresher.refresh_system(&pids_to_refresh).await?;

        // Step 4: Log processes
        let mut logged_count = 0;
        for (target, processes) in &matched_processes {
            for process in processes {
                let system = system_refresher.get_system().read().await;
                let sys_proc = system.process(process.pid.into());

                logger.log_new_process(target, process, sys_proc).await?;
                logged_count += 1;
            }
        }
        debug!("Logged {} processes.", logged_count);

        // Step 5: Set up ongoing monitoring
        state_manager.update_monitoring(matched_processes).await?;
        debug!("Monitoring updated.");

        Ok(())
    }


}
