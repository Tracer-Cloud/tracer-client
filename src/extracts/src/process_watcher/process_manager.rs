use crate::process_watcher::ProcessState;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracer_common::recorder::LogRecorder;
use tracer_common::target_process::manager::TargetManager;
use tracer_common::target_process::Target;
use tracer_common::types::ebpf_trigger::{
    ExitReason, OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger,
};
use tracer_common::types::event::attributes::process::CompletedProcess;
use tracer_common::types::event::attributes::EventAttributes;
use tracer_common::types::event::ProcessStatus as TracerProcessStatus;
use tracing::{debug, error};

pub struct ProcessManager {
    state: Arc<RwLock<ProcessState>>,
    log_recorder: LogRecorder,
}

impl ProcessManager {
    pub fn new(target_manager: TargetManager, log_recorder: LogRecorder) -> Self {
        let state = Arc::new(RwLock::new(ProcessState {
            processes: HashMap::new(),
            monitoring: HashMap::new(),
            target_manager,
            ebpf_task: None,
            oom_victims: HashMap::new(),
        }));

        ProcessManager {
            state,
            log_recorder,
        }
    }

    /// Gets a read lock on the process state
    pub async fn get_state(&self) -> RwLockReadGuard<ProcessState> {
        self.state.read().await
    }

    /// Gets a write lock on the process state
    pub async fn get_state_mut(&self) -> RwLockWriteGuard<ProcessState> {
        self.state.write().await
    }

    /// Sets a new process state
    pub async fn set_state(&self, new_state: ProcessState) {
        let mut state = self.state.write().await;
        *state = new_state;
    }

    /// Updates the list of targets being watched
    pub async fn update_targets(&self, targets: Vec<Target>) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        state.target_manager.targets = targets;
        Ok(())
    }

    /// Enriches finish triggers with OOM reason if they were OOM victims
    pub async fn handle_out_of_memory_terminations(
        &self,
        finish_triggers: &mut [ProcessEndTrigger],
    ) {
        let mut state = self.state.write().await;

        for finish in finish_triggers.iter_mut() {
            if state.oom_victims.remove(&finish.pid).is_some() {
                finish.exit_reason = Some(ExitReason::OutOfMemoryKilled);
                debug!("Marked PID {} as OOM-killed", finish.pid);
            }
        }
    }

    pub async fn handle_out_of_memory_signals(
        &self,
        triggers: Vec<OutOfMemoryTrigger>,
    ) -> HashMap<usize, OutOfMemoryTrigger> {
        let mut victims = HashMap::new();
        let mut state = self.state.write().await;

        for oom in triggers {
            let is_related = state.processes.contains_key(&oom.pid)
                || state.processes.values().any(|p| p.ppid == oom.pid);

            if is_related {
                debug!("Tracking OOM for relevant pid {}", oom.pid);
                victims.insert(oom.pid, oom.clone());
                state.oom_victims.insert(oom.pid, oom);
            } else {
                debug!("Ignoring unrelated OOM for pid {}", oom.pid);
            }
        }

        victims
    }

    async fn remove_processes_from_state(
        &self,
        triggers: &[ProcessEndTrigger],
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        for trigger in triggers.iter() {
            state.processes.remove(&trigger.pid);
        }
        Ok(())
    }

    async fn handle_process_terminations(
        &self,
        triggers: Vec<ProcessEndTrigger>,
    ) -> anyhow::Result<()> {
        debug!("Processing {} process terminations", triggers.len());

        // Remove terminated processes from the state
        self.remove_processes_from_state(&triggers).await?;

        // Map PIDs to finish triggers for easy lookup
        let mut pid_to_finish: HashMap<_, _> =
            triggers.into_iter().map(|proc| (proc.pid, proc)).collect();

        // Find all processes that we were monitoring that have terminated
        let terminated_processes: HashSet<_> = {
            let mut state = self.state.write().await;

            state
                .monitoring
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
            // should be safe since
            // - we've checked the key is present
            // - we have an exclusive lock on the state
            // - if trigger is duplicated in monitoring (can happen if it matches several targets),
            //   it'll be deduplicated via hashset

            self.log_process_completion(&start_trigger, &finish_trigger)
                .await?;
        }

        Ok(())
    }

    async fn log_process_completion(
        &self,
        start_trigger: &ProcessStartTrigger,
        finish_trigger: &ProcessEndTrigger,
    ) -> anyhow::Result<()> {
        let duration_sec = (finish_trigger.finished_at - start_trigger.started_at)
            .num_seconds()
            .try_into()
            .unwrap_or(0);

        let properties = CompletedProcess {
            tool_name: start_trigger.comm.clone(),
            tool_pid: start_trigger.pid.to_string(),
            duration_sec,
            exit_reason: finish_trigger.exit_reason.clone(),
        };

        self.log_recorder
            .log(
                TracerProcessStatus::FinishedToolExecution,
                format!("[{}] {} exited", Utc::now(), &start_trigger.comm),
                Some(EventAttributes::CompletedProcess(properties)),
                None,
            )
            .await?;

        Ok(())
    }
}
