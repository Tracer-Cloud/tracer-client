use crate::extracts::process::process_manager::handlers::oom::OomHandler;
use crate::extracts::process::process_manager::handlers::process_starts::ProcessStartHandler;
use crate::extracts::process::process_manager::handlers::process_terminations::ProcessTerminationHandler;
use crate::extracts::process::process_manager::logger::ProcessLogger;
use crate::extracts::process::process_manager::matcher::Filter;
use crate::extracts::process::process_manager::metrics::ProcessMetricsHandler;
use crate::extracts::process::process_manager::state::StateManager;
use crate::extracts::process::process_manager::system_refresher::SystemRefresher;
use crate::process_identification::recorder::LogRecorder;
use crate::process_identification::target_process::target::Target;
use crate::process_identification::target_process::target_manager::TargetManager;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tokio::task::JoinHandle;
use tracer_ebpf::ebpf_trigger::{OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger};

/// Main coordinator for process management operations
/// Uses functional programming principles with direct component access
pub struct ProcessManager {
    pub state_manager: StateManager,
    pub logger: ProcessLogger,
    pub matcher: Filter,
    pub system_refresher: SystemRefresher,
}

impl ProcessManager {
    pub fn new(target_manager: TargetManager, log_recorder: LogRecorder) -> Self {
        let state_manager = StateManager::new(target_manager);
        let logger = ProcessLogger::new(log_recorder);
        let matcher = Filter::new();
        let system_refresher = SystemRefresher::new();

        ProcessManager {
            state_manager,
            logger,
            matcher,
            system_refresher,
        }
    }

    /// Sets the eBPF task handle
    pub async fn set_ebpf_task(&self, task: JoinHandle<()>) {
        self.state_manager.set_ebpf_task(task).await;
    }

    /// Updates the list of targets being watched
    pub async fn update_targets(&self, targets: Vec<Target>) -> Result<()> {
        // StateManager is now the single source of truth for targets
        self.state_manager.update_targets(targets).await
    }

    /// Handles out-of-memory terminations
    pub async fn handle_out_of_memory_terminations(
        &self,
        finish_triggers: &mut [ProcessEndTrigger],
    ) {
        OomHandler::handle_out_of_memory_terminations(&self.state_manager, finish_triggers).await;
    }

    /// Handles out-of-memory signals
    pub async fn handle_out_of_memory_signals(
        &self,
        triggers: Vec<OutOfMemoryTrigger>,
    ) -> HashMap<usize, OutOfMemoryTrigger> {
        OomHandler::handle_out_of_memory_signals(&self.state_manager, triggers).await
    }

    /// Handles process terminations
    pub async fn handle_process_terminations(
        &self,
        triggers: Vec<ProcessEndTrigger>,
    ) -> Result<()> {
        ProcessTerminationHandler::handle_process_terminations(
            &self.state_manager,
            &self.logger,
            triggers,
        )
        .await
    }

    /// Handles newly started processes
    pub async fn handle_process_starts(&self, triggers: Vec<ProcessStartTrigger>) -> Result<()> {
        ProcessStartHandler::handle_process_starts(
            &self.state_manager,
            &self.logger,
            &self.matcher,
            &self.system_refresher,
            triggers,
        )
        .await
    }

    /// Polls and updates metrics for all monitored processes
    pub async fn poll_process_metrics(&self) -> Result<()> {
        ProcessMetricsHandler::poll_process_metrics(
            &self.state_manager,
            &self.logger,
            &self.system_refresher,
        )
        .await
    }

    /// Returns N process names of monitored processes
    pub async fn get_n_monitored_processes(&self, n: usize) -> HashSet<String> {
        self.state_manager.get_n_monitored_processes(n).await
    }

    /// Returns the total number of processes being monitored
    pub async fn get_number_of_monitored_processes(&self) -> usize {
        self.state_manager.get_number_of_monitored_processes().await
    }
}
