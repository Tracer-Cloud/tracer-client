use crate::extracts::process::process_manager::logger::ProcessLogger;
use crate::extracts::process::process_manager::state::StateManager;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tracer_ebpf::ebpf_trigger::ProcessEndTrigger;
use tracing::{debug, error};

/// Handles process termination events
pub struct ProcessTerminationHandler;

impl ProcessTerminationHandler {
    /// Handles process terminations by removing them from state and logging completion
    pub async fn handle_process_terminations(
        state_manager: &StateManager,
        logger: &ProcessLogger,
        triggers: Vec<ProcessEndTrigger>,
    ) -> Result<()> {
        debug!("Processing {} process terminations", triggers.len());

        // Remove terminated processes from the state
        Self::remove_processes_from_state(state_manager, &triggers).await?;

        // Map PIDs to finish triggers for easy lookup
        let mut pid_to_finish: HashMap<_, _> =
            triggers.into_iter().map(|proc| (proc.pid, proc)).collect();

        // Find all processes that we were monitoring that have terminated
        let terminated_processes: HashSet<_> = {
            let mut state = state_manager.get_state_mut().await;
            let monitoring = state.get_monitoring_mut();

            monitoring
                .iter_mut()
                .flat_map(|(_, procs)| {
                    // Partition processes into terminated and still running
                    let (terminated, still_running): (Vec<_>, Vec<_>) = procs
                        .drain()
                        .partition(|proc| pid_to_finish.contains_key(&proc.pid));

                    // Update monitoring with still running processes
                    *procs = still_running.into_iter().collect();

                    // Return terminated processes
                    terminated
                })
                .collect()
        };

        debug!(
            "Removed {} processes. terminated={:?}, pid_to_finish={:?}",
            terminated_processes.len(),
            terminated_processes,
            pid_to_finish
        );

        // Log completion events for each terminated process
        for start_trigger in terminated_processes {
            let Some(finish_trigger) = pid_to_finish.remove(&start_trigger.pid) else {
                error!("Process doesn't exist: start_trigger={:?}", start_trigger);
                continue;
            };

            logger.log_process_completion(&start_trigger, &finish_trigger).await?;
        }

        Ok(())
    }

    /// Removes terminated processes from the state
    async fn remove_processes_from_state(
        state_manager: &StateManager,
        triggers: &[ProcessEndTrigger],
    ) -> Result<()> {
        for trigger in triggers.iter() {
            state_manager.remove_process(&trigger.pid).await;
        }
        Ok(())
    }
}
