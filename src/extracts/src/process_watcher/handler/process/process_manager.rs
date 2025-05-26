use crate::process_watcher::handler::process::extract_process_data::ExtractProcessData;
use chrono::Utc;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sysinfo::{Pid, ProcessRefreshKind, System};
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio::task::JoinHandle;
use tracer_common::recorder::LogRecorder;
use tracer_common::target_process::manager::TargetManager;
use tracer_common::target_process::{Target, TargetMatchable};
use tracer_common::types::ebpf_trigger::{
    ExitReason, OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger,
};
use tracer_common::types::event::attributes::process::{
    CompletedProcess, ProcessProperties, ShortProcessProperties,
};
use tracer_common::types::event::attributes::EventAttributes;
use tracer_common::types::event::ProcessStatus as TracerProcessStatus;
use tracing::{debug, error};

/// Internal state of the process manager
pub struct ProcessState {
    // Maps PIDs to process triggers
    processes: HashMap<usize, ProcessStartTrigger>,
    // Maps targets to sets of processes being monitored
    monitoring: HashMap<Target, HashSet<ProcessStartTrigger>>,
    // List of targets to watch
    target_manager: TargetManager,
    // Store task handle to ensure it stays alive
    ebpf_task: Option<tokio::task::JoinHandle<()>>,
    // tracks relevant processes killed with oom
    oom_victims: HashMap<usize, OutOfMemoryTrigger>, // Map of pid -> oom trigger
}

enum ProcessResult {
    NotFound,
    Found,
}

pub struct ProcessManager {
    state: Arc<RwLock<ProcessState>>,
    log_recorder: LogRecorder,
    system: Arc<RwLock<System>>,
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

        let system = Arc::new(RwLock::new(System::new_all()));

        ProcessManager {
            state,
            log_recorder,
            system,
        }
    }

    /// Gets a write lock on the process state
    pub async fn get_state_mut(&self) -> RwLockWriteGuard<ProcessState> {
        self.state.write().await
    }

    pub async fn set_ebpf_task(&mut self, task: JoinHandle<()>) {
        let mut state = self.get_state_mut().await;
        state.ebpf_task = Some(task);
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

    pub async fn handle_process_terminations(
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

    pub async fn handle_process_starts(
        &self,
        triggers: Vec<ProcessStartTrigger>,
    ) -> anyhow::Result<()> {
        debug!("Processing {} process starts", triggers.len());

        // Find processes we're interested in based on targets
        let interested_in = self.filter_processes_of_interest(triggers).await?;

        debug!(
            "After filtering, interested in {} processes",
            interested_in.len()
        );

        if interested_in.is_empty() {
            return Ok(());
        }

        // Get the set of PIDs to refresh system data for
        let pids_to_refresh = interested_in
            .values()
            .flat_map(|procs| procs.iter().map(|p| p.pid))
            .collect();

        // Refresh system data for these processes
        self.refresh_system(&pids_to_refresh).await?;

        // Process each new process
        for (target, triggers) in interested_in.iter() {
            for process in triggers.iter() {
                self.handle_new_process(target, process).await?;
            }
        }

        // Update monitoring state with new processes
        let mut state = self.state.write().await;
        for (target, processes) in interested_in.into_iter() {
            state
                .monitoring
                .entry(target)
                .or_default()
                .extend(processes);
        }

        Ok(())
    }

    async fn filter_processes_of_interest(
        &self,
        triggers: Vec<ProcessStartTrigger>,
    ) -> anyhow::Result<HashMap<Target, HashSet<ProcessStartTrigger>>> {
        // Store all triggers in the state
        {
            let mut state = self.state.write().await;
            for trigger in triggers.iter() {
                state.processes.insert(trigger.pid, trigger.clone());
            }
        }

        // Get PIDs of processes already being monitored
        let state = self.state.read().await;
        let already_monitored_pids: HashSet<usize> = state
            .monitoring
            .values()
            .flat_map(|processes| processes.iter().map(|p| p.pid))
            .collect();

        // Find processes that match our targets
        let matched_processes = self.find_matching_processes(triggers).await?;

        // Filter out already monitored processes and include parent processes
        let interested_in: HashMap<_, _> = matched_processes
            .into_iter()
            .map(|(target, processes)| {
                let processes = processes
                    .into_iter()
                    .flat_map(|proc| {
                        // Get the process and its parents
                        let mut parents = self.get_process_hierarchy(&state, proc);
                        // Filter out already monitored processes
                        parents.retain(|p| !already_monitored_pids.contains(&p.pid));
                        parents
                    })
                    .collect::<HashSet<_>>();

                (target, processes)
            })
            .collect();

        Ok(interested_in)
    }

    /// Refreshes system information for the specified PIDs
    ///
    /// Uses tokio's spawn_blocking to execute the potentially blocking refresh operation
    /// without affecting the async runtime.
    #[tracing::instrument(skip(self))]
    async fn refresh_system(&self, pids: &HashSet<usize>) -> anyhow::Result<()> {
        // Convert PIDs to the format expected by sysinfo
        let pids_vec = pids.iter().map(|pid| Pid::from(*pid)).collect::<Vec<_>>();

        // Clone the PIDs vector since we need to move it into the spawn_blocking closure
        let pids_for_closure = pids_vec.clone();

        // Get a mutable reference to the system
        let system = Arc::clone(&self.system);

        // Execute the blocking operation in a separate thread
        tokio::task::spawn_blocking(move || {
            let mut sys = system.blocking_write();
            sys.refresh_pids_specifics(
                pids_for_closure.as_slice(),
                ProcessRefreshKind::everything(), // TODO(ENG-336): minimize data collected for performance
            );
        })
        .await?;

        Ok(())
    }

    /// Gets a process and all its parent processes from the state
    ///
    /// Will panic if a cycle is detected in the process hierarchy.
    pub fn get_process_hierarchy(
        &self,
        state: &ProcessState,
        process: ProcessStartTrigger,
    ) -> HashSet<ProcessStartTrigger> {
        let mut current_pid = process.ppid;
        let mut hierarchy = HashSet::new();
        // Keep track of visited PIDs to detect cycles
        let mut visited_pids = HashSet::new();

        // Store the process PID before moving the process
        let process_pid = process.pid;

        // Insert the process into the hierarchy (this moves the process)
        hierarchy.insert(process);

        // Add the starting process PID to visited
        visited_pids.insert(process_pid);

        // Traverse up the process tree to include all parent processes
        while let Some(parent) = state.processes.get(&current_pid) {
            // Check if we've seen this PID before - that would indicate a cycle
            if visited_pids.contains(&parent.pid) {
                // We have a cycle in the process hierarchy - this shouldn't happen
                // in normal scenarios, but we'll panic to prevent infinite loops
                panic!(
                    "Cycle detected in process hierarchy! PID {} appears twice in parent chain",
                    parent.pid
                );
            }

            // Track that we've visited this PID
            visited_pids.insert(parent.pid);

            // Add parent to the hierarchy
            hierarchy.insert(parent.clone());

            // Move to the next parent
            current_pid = parent.ppid;
        }

        hierarchy
    }

    /// Gets a process and all its parent processes from the state
    ///
    /// Will panic if a cycle is detected in the process hierarchy.
    fn get_process_parents<'a>(
        state: &'a ProcessState,
        process: &'a ProcessStartTrigger,
    ) -> HashSet<&'a ProcessStartTrigger> {
        let mut current_pid = process.ppid;
        let mut hierarchy = HashSet::new();
        // Keep track of visited PIDs to detect cycles
        let mut visited_pids = HashSet::new();

        // Store the process PID before moving the process
        let process_pid = process.pid;

        // Insert the process into the hierarchy (this moves the process)
        hierarchy.insert(process);

        // Add the starting process PID to visited
        visited_pids.insert(process_pid);

        // Traverse up the process tree to include all parent processes
        while let Some(parent) = state.processes.get(&current_pid) {
            // Check if we've seen this PID before - that would indicate a cycle
            if visited_pids.contains(&parent.pid) {
                // We have a cycle in the process hierarchy - this shouldn't happen
                // in normal scenarios, but we'll panic to prevent infinite loops
                panic!(
                    "Cycle detected in process hierarchy! PID {} appears twice in parent chain",
                    parent.pid
                );
            }

            // Track that we've visited this PID
            visited_pids.insert(parent.pid);

            // Add parent to the hierarchy
            hierarchy.insert(parent);

            // Move to the next parent
            current_pid = parent.ppid;
        }

        hierarchy
    }

    pub async fn find_matching_processes(
        &self,
        triggers: Vec<ProcessStartTrigger>,
    ) -> anyhow::Result<HashMap<Target, HashSet<ProcessStartTrigger>>> {
        let state = self.state.read().await;
        let mut matched_processes = HashMap::new();

        for trigger in triggers {
            if let Some(matched_target) = Self::get_matched_target(&state, &trigger) {
                let matched_target = matched_target.clone(); // todo: remove clone, or move targets to arcs?
                matched_processes
                    .entry(matched_target)
                    .or_insert(HashSet::new())
                    .insert(trigger);
            }
        }

        Ok(matched_processes)
    }

    fn get_matched_target<'a>(
        state: &'a ProcessState,
        process: &ProcessStartTrigger,
    ) -> Option<&'a Target> {
        if let Some(target) = state.target_manager.get_target_match(process) {
            return Some(target);
        }

        let eligible_targets_for_parents = state
            .target_manager
            .targets
            .iter()
            .filter(|target| !target.should_force_ancestor_to_match())
            .collect_vec();

        if eligible_targets_for_parents.is_empty() {
            return None;
        }

        // Here it's tempting to check if the parent is just in the monitoring list. However, we can't do that because
        // parent may be matching but not yet set to be monitoring (e.g., because it just arrived or even is in the same batch)

        let parents = Self::get_process_parents(state, process);
        for parent in parents {
            for target in eligible_targets_for_parents.iter() {
                if target.matches_process(parent) {
                    return Some(target);
                }
            }
        }

        None
    }

    async fn handle_new_process(
        &self,
        target: &Target,
        process: &ProcessStartTrigger,
    ) -> anyhow::Result<ProcessResult> {
        debug!("Processing pid={}", process.pid);

        let display_name = target
            .get_display_name_object()
            .get_display_name(&process.file_name, process.argv.as_slice());

        let properties = {
            let system = self.system.read().await;

            match system.process(process.pid.into()) {
                Some(system_process) => {
                    ExtractProcessData::gather_process_data(
                        system_process,
                        display_name.clone(),
                        process.started_at,
                    )
                    .await
                }
                None => {
                    debug!("Process({}) wasn't found", process.pid);
                    self.create_short_lived_process_properties(process, display_name.clone())
                }
            }
        };

        self.log_recorder
            .log(
                TracerProcessStatus::ToolExecution,
                format!("[{}] Tool process: {}", Utc::now(), &display_name),
                Some(EventAttributes::Process(properties)),
                None,
            )
            .await?;

        Ok(ProcessResult::Found)
    }

    /// Creates properties for a short-lived process that wasn't found in the system
    fn create_short_lived_process_properties(
        &self,
        process: &ProcessStartTrigger,
        display_name: String,
    ) -> ProcessProperties {
        ProcessProperties::ShortLived(Box::new(ShortProcessProperties {
            tool_name: display_name,
            tool_pid: process.pid.to_string(),
            tool_parent_pid: process.ppid.to_string(),
            tool_binary_path: process.file_name.clone(),
            start_timestamp: Utc::now().to_rfc3339(),
        }))
    }

    /// Processes an already running process for metrics updates
    async fn update_running_process(
        &self,
        target: &Target,
        process: &ProcessStartTrigger,
    ) -> anyhow::Result<ProcessResult> {
        let display_name = target
            .get_display_name_object()
            .get_display_name(&process.file_name, process.argv.as_slice());

        let properties = {
            let system = self.system.read().await;

            let Some(system_process) = system.process(process.pid.into()) else {
                // Process no longer exists
                return Ok(ProcessResult::NotFound);
            };

            debug!(
                "Loaded process. PID: ebpf={}, system={:?}; Start Time: ebpf={}, system={:?};",
                process.pid,
                system_process.pid(),
                process.started_at.timestamp(),
                system_process.start_time()
            );

            // Don't process input files for update events
            ExtractProcessData::gather_process_data(
                system_process,
                display_name.clone(),
                process.started_at,
            )
            .await
        };

        debug!("Process data completed. PID={}", process.pid);

        self.log_recorder
            .log(
                TracerProcessStatus::ToolMetricEvent,
                format!("[{}] Tool metric event: {}", Utc::now(), &display_name),
                Some(EventAttributes::Process(properties)),
                None,
            )
            .await?;

        Ok(ProcessResult::Found)
    }

    /// Polls and updates metrics for all monitored processes
    pub async fn poll_process_metrics(&self) -> anyhow::Result<()> {
        debug!("Polling process metrics");

        // Get PIDs of all monitored processes
        let pids = {
            let state = self.state.read().await;
            debug!("Refreshing data for {} processes", state.monitoring.len());

            if state.monitoring.is_empty() {
                debug!("No processes to monitor, skipping poll");
                return Ok(());
            }

            state
                .monitoring
                .iter()
                .flat_map(|(_, processes)| processes.iter().map(|p| p.pid))
                .collect::<HashSet<_>>()
        };

        // Refresh system data and process updates
        self.refresh_system(&pids).await?;
        self.update_all_processes().await?;

        debug!("Refreshing data completed");

        Ok(())
    }

    /// Returns N process names of monitored processes
    pub async fn preview_targets(&self, n: usize) -> HashSet<String> {
        self.state
            .read()
            .await
            .monitoring
            .iter()
            .flat_map(|(_, processes)| processes.iter().map(|p| p.comm.clone()))
            .take(n)
            .collect()
    }

    /// Returns the total number of processes being monitored
    pub async fn targets_len(&self) -> usize {
        self.state
            .read()
            .await
            .monitoring
            .values()
            .map(|processes| processes.len())
            .sum()
    }

    /// Updates all monitored processes with fresh data
    #[tracing::instrument(skip(self))]
    async fn update_all_processes(&self) -> anyhow::Result<()> {
        for (target, procs) in self.state.read().await.monitoring.iter() {
            for proc in procs.iter() {
                let result = self.update_running_process(target, proc).await?;

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
}
