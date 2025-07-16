use crate::extracts::process::process_manager::matcher::Filter;
use crate::extracts::process::process_manager::state::StateManager;
use crate::extracts::process::process_manager::system_refresher::SystemRefresher;
use crate::{
    constants::PROCESS_POLLING_INTERVAL_MS,
    extracts::process::process_manager::logger::ProcessLogger,
};
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
        matcher: &Filter,
        system_refresher: &SystemRefresher,
        triggers: Vec<ProcessStartTrigger>,
    ) -> Result<()> {
        let total_triggers = triggers.len();
        debug!("Handling {} process start triggers", total_triggers);

        let existing = {
            let guard = state_manager.get_state().await;
            guard.get_processes().clone()
        };

        let unique_triggers = Self::filter_unique_triggers(
            triggers,
            existing.values().cloned(),
            PROCESS_POLLING_INTERVAL_MS as i64,
        );

        debug!(
            "Filtered {} duplicates; proceeding with {} unique triggers",
            total_triggers - unique_triggers.len(),
            unique_triggers.len()
        );

        Self::store_triggers(state_manager, unique_triggers.clone()).await;

        let matched_processes =
            Self::match_processes(state_manager, matcher, unique_triggers).await;

        if matched_processes.is_empty() {
            debug!("No matching processes found; exiting early.");
            return Ok(());
        }

        // TODO: refresh_process_data doesn't modify matched_processes - it should take a reference
        // and not return anything
        let refreshed_processes =
            Self::refresh_process_data(system_refresher, matched_processes).await?;

        Self::log_matched_processes(logger, system_refresher, &refreshed_processes).await?;

        Self::update_monitoring(state_manager, refreshed_processes).await?;

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
    async fn match_processes(
        state_manager: &StateManager,
        matcher: &Filter,
        triggers: Vec<ProcessStartTrigger>,
    ) -> HashMap<String, HashSet<ProcessStartTrigger>> {
        debug!(
            "Matching {} stored triggers against targets.",
            triggers.len()
        );
        let state = state_manager.get_state().await;
        matcher.find_matching_processes(triggers, &state)
    }

    /// Step 3: Refresh system data for matched processes.
    async fn refresh_process_data(
        system_refresher: &SystemRefresher,
        matched_processes: HashMap<String, HashSet<ProcessStartTrigger>>,
    ) -> Result<HashMap<String, HashSet<ProcessStartTrigger>>> {
        let pids: HashSet<usize> = matched_processes
            .values()
            .flatten()
            .map(|trigger| trigger.pid)
            .collect();

        debug!("Refreshing system data for {} PIDs.", pids.len());
        system_refresher.refresh_system(&pids).await?;

        Ok(matched_processes)
    }

    /// Step 4: Log data for each matched process.
    async fn log_matched_processes(
        logger: &ProcessLogger,
        system_refresher: &SystemRefresher,
        matched_processes: &HashMap<String, HashSet<ProcessStartTrigger>>,
    ) -> Result<()> {
        let mut count = 0;

        for (target, processes) in matched_processes {
            count += processes.len();
            for process in processes {
                let system = system_refresher.get_system().read().await;
                let sys_proc = system.process(process.pid.into());
                logger.log_new_process(target, process, sys_proc).await?;
            }
        }

        debug!("Logged data for {} matched processes.", count);
        Ok(())
    }

    /// Step 5: Update the monitoring state with new processes.
    async fn update_monitoring(
        state_manager: &StateManager,
        matched_processes: HashMap<String, HashSet<ProcessStartTrigger>>,
    ) -> Result<()> {
        debug!("Updating monitoring for matched processes.");
        state_manager.update_monitoring(matched_processes).await
    }

    fn filter_unique_triggers(
        incoming: Vec<ProcessStartTrigger>,
        mut existing: impl Iterator<Item = ProcessStartTrigger>,
        max_ms_drift: i64,
    ) -> Vec<ProcessStartTrigger> {
        let mut unique = Vec::new();
        let mut seen: Vec<ProcessStartTrigger> = Vec::new();

        for inc in incoming {
            let is_duplicate = existing.by_ref().chain(seen.iter().cloned()).any(|stored| {
                stored.pid == inc.pid
                    && stored.command_string == inc.command_string
                    && (stored.started_at.timestamp_millis() - inc.started_at.timestamp_millis())
                        .abs()
                        <= max_ms_drift
            });

            if !is_duplicate {
                seen.push(inc.clone());
                unique.push(inc);
            }
        }

        unique
    }
}
