use crate::extracts::process::process_manager::logger::ProcessLogger;
use crate::extracts::process::process_manager::matcher::Filter;
use crate::extracts::process::process_manager::state::StateManager;
use crate::extracts::process::process_manager::system_refresher::SystemRefresher;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;
use tracing::debug;

/// Handles process start events through explicit data transformations.
pub struct ProcessStartHandler;

impl ProcessStartHandler {
    /// Entry point: handles newly started processes.
    pub async fn handle_process_starts(
        state_manager: &StateManager,
        logger: &ProcessLogger,
        system_refresher: &SystemRefresher,
        triggers: Vec<ProcessStartTrigger>,
    ) -> Result<()> {
        debug!("Handling {} process start triggers", triggers.len());

        Self::store_triggers(state_manager, triggers.clone()).await;

        let matched_processes = Self::match_processes(state_manager, matcher, triggers).await;

        if matched_processes.is_empty() {
            debug!("No matching processes found; exiting early.");
            return Ok(());
        }

        Self::refresh_process_data(system_refresher, &matched_processes).await?;

        Self::log_matched_processes(logger, system_refresher, &matched_processes).await?;

        Self::log_matching_tasks(logger, state_manager, &triggers, &matched_processes).await?;

        Self::update_monitoring(state_manager, matched_processes).await?;

        debug!("Process start handling completed successfully.");

        Ok(())
    }

    /// Step 1: Store triggers for parent-child tracking in state.
    async fn store_triggers(state_manager: &StateManager, triggers: Vec<ProcessStartTrigger>) {
        debug!("Storing {} triggers in state.", triggers.len());
        for trigger in &triggers {
            state_manager
                .insert_process(trigger.pid, trigger.clone())
                .await;
        }
    }

    /// Step 2: Match stored triggers against targets.
    async fn match_processes<'a>(
        state_manager: &StateManager,
        triggers: &'a [ProcessStartTrigger],
    ) -> HashMap<String, HashSet<&'a ProcessStartTrigger>> {
        debug!(
            "Matching {} stored triggers against targets.",
            triggers.len()
        );
        let state = state_manager.get_state().await;
        Filter.find_matching_processes(triggers, &state)
    }

    /// Step 3: Refresh system data for matched processes.
    async fn refresh_process_data(
        system_refresher: &SystemRefresher,
        matched_processes: &HashMap<String, HashSet<&ProcessStartTrigger>>,
    ) -> Result<()> {
        let pids: HashSet<usize> = matched_processes
            .values()
            .flatten()
            .map(|trigger| trigger.pid)
            .collect();

        debug!("Refreshing system data for {} PIDs.", pids.len());
        system_refresher.refresh_system(&pids).await
    }

    /// Step 4: Log data for each matched process.
    async fn log_matched_processes(
        logger: &ProcessLogger,
        system_refresher: &SystemRefresher,
        matched_processes: &HashMap<String, HashSet<&ProcessStartTrigger>>,
    ) -> Result<()> {
        let mut count = 0;

        for (target, processes) in matched_processes {
            count += processes.len();
            for process in processes {
                let system = system_refresher.get_system().read().await;
                let sys_proc = system.process(process.pid.into());
                let _ = logger.log_new_process(target, process, sys_proc).await?;
            }
        }

        debug!("Logged data for {} matched processes.", count);
        Ok(())
    }

    /// Step 5: Match pipelines for matched processes.
    async fn log_matching_tasks(
        logger: &ProcessLogger,
        state_manager: &StateManager,
        triggers: &Vec<ProcessStartTrigger>,
        matched_processes: &HashMap<String, HashSet<&ProcessStartTrigger>>,
    ) -> Result<()> {
        let mut state = state_manager.get_state_mut().await;
        let pipeline_manager = state.get_pipeline_manager();
        let trigger_to_target =
            matched_processes
                .iter()
                .fold(HashMap::new(), |mut acc, (target, processes)| {
                    acc.extend(processes.iter().map(|process| (process, target)));
                    acc
                });
        for trigger in triggers {
            let matched_target = trigger_to_target.get(&trigger);
            if let Some(task_match) =
                pipeline_manager.register_process(trigger, matched_target.map(|t| &**t))
            {
                // the process triggered a task match
                logger.log_task_match(task_match).await?;
            }
        }
        Ok(())
    }

    /// Step 6: Update the monitoring state with new processes.
    async fn update_monitoring(
        state_manager: &StateManager,
        matched_processes: HashMap<String, HashSet<&ProcessStartTrigger>>,
    ) -> Result<()> {
        debug!("Updating monitoring for matched processes.");
        let matched_processes = matched_processes
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().cloned().collect()))
            .collect();
        state_manager.update_monitoring(matched_processes).await
    }
}
