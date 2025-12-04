use crate::extracts::process::process_manager::handlers::process_starts::ProcessStartHandler;
use crate::extracts::process::process_manager::handlers::process_terminations::ProcessTerminationHandler;
use crate::extracts::process::process_manager::metrics::ProcessMetricsHandler;
use crate::extracts::process::process_manager::recorder::EventRecorder;
use crate::extracts::process::process_manager::state::StateManager;
use crate::extracts::process::process_manager::system_refresher::SystemRefresher;
use crate::extracts::{
    containers::DockerWatcher, process::process_manager::handlers::oom::OomHandler,
};
use crate::process_identification::recorder::EventDispatcher;
use anyhow::Result;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::task::JoinHandle;
use tracer_ebpf::ebpf_trigger::{
    FileOpenTrigger, OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger,
};
use tracing::debug;

/// Main coordinator for process management operations
/// Uses functional programming principles with direct component access
pub struct ProcessManager {
    pub state_manager: StateManager,
    pub event_recorder: EventRecorder,
    pub system_refresher: SystemRefresher,
}

impl ProcessManager {
    pub fn new(event_dispatcher: EventDispatcher, docker_watcher: Arc<DockerWatcher>) -> Self {
        let state_manager = StateManager::default();
        let event_recorder = EventRecorder::new(event_dispatcher, docker_watcher);
        let system_refresher = SystemRefresher::new();

        ProcessManager {
            state_manager,
            event_recorder,
            system_refresher,
        }
    }

    /// Sets the eBPF task handle
    pub async fn set_ebpf_task(&self, task: JoinHandle<()>) {
        self.state_manager.set_ebpf_task(task).await;
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
            &self.event_recorder,
            triggers,
        )
        .await
    }

    /// Handles newly started processes
    pub async fn handle_process_starts(&self, triggers: Vec<ProcessStartTrigger>) -> Result<()> {
        ProcessStartHandler::handle_process_starts(
            &self.state_manager,
            &self.event_recorder,
            &self.system_refresher,
            triggers,
        )
        .await
    }

    pub async fn handle_file_openings(
        &self,
        file_opening_triggers: Vec<FileOpenTrigger>,
    ) -> Result<()> {
        debug!("handle_file_opening: {}", file_opening_triggers.len());
        Ok(())
    }

    /// Polls and updates metrics for all monitored processes
    pub async fn poll_process_metrics(&self) -> Result<()> {
        ProcessMetricsHandler::poll_process_metrics(
            &self.state_manager,
            &self.event_recorder,
            &self.system_refresher,
        )
        .await
    }

    /// Returns a set of monitored process names
    pub async fn get_monitored_processes(&self) -> HashSet<String> {
        self.state_manager.get_monitored_processes().await
    }

    /// Returns a set of matched tasks
    pub async fn get_matched_tasks(&self) -> HashMap<String, usize> {
        self.state_manager.get_matched_tasks().await
    }
}
