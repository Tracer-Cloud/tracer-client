use tracer_common::types::event::ProcessStatus as TracerProcessStatus;

use crate::data_samples::DATA_SAMPLES_EXT;
use crate::file_watcher::FileWatcher;
use anyhow::Result;
use chrono::Utc;
use itertools::Itertools;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessStatus, System};
use tokio::sync::{mpsc, RwLock};
use tracer_common::recorder::LogRecorder;
use tracer_common::target_process::{Target, TargetMatchable};
use tracer_common::types::event::attributes::process::{
    CompletedProcess, DataSetsProcessed, FullProcessProperties, InputFile, ProcessProperties,
    ShortProcessProperties,
};
use tracer_common::types::event::attributes::EventAttributes;
use tracer_common::types::trigger::{FinishTrigger, ProcessTrigger, Trigger};
use tracer_ebpf_user::{start_processing_events, TracerEbpf};
use tracing::{debug, error};

enum ProcessResult {
    NotFound,
    Found,
}

fn process_status_to_string(status: &ProcessStatus) -> String {
    match status {
        ProcessStatus::Run => "Run".to_string(),
        ProcessStatus::Sleep => "Sleep".to_string(),
        ProcessStatus::Idle => "Idle".to_string(),
        ProcessStatus::Zombie => "Zombie".to_string(),
        ProcessStatus::Stop => "Stop".to_string(),
        ProcessStatus::Parked => "Parked".to_string(),
        ProcessStatus::Tracing => "Tracing".to_string(),
        ProcessStatus::Dead => "Dead".to_string(),
        ProcessStatus::UninterruptibleDiskSleep => "Uninterruptible Disk Sleep".to_string(),
        ProcessStatus::Waking => "Waking".to_string(),
        ProcessStatus::LockBlocked => "Lock Blocked".to_string(),
        _ => "Unknown".to_string(),
    }
}

/// Internal state of the process watcher
struct ProcessState {
    // Maps PIDs to process triggers
    processes: HashMap<usize, ProcessTrigger>,
    // Maps targets to sets of processes being monitored
    monitoring: HashMap<Target, HashSet<ProcessTrigger>>,
    // Groups datasets by the nextflow session UUID
    datasamples_tracker: HashMap<String, HashSet<String>>,
    // List of targets to watch
    targets: Vec<Target>,
}

/// Watches system processes and records events related to them
pub struct ProcessWatcher {
    ebpf: once_cell::sync::OnceCell<TracerEbpf>, // not tokio, because TracerEbpf is sync
    log_recorder: LogRecorder,
    file_watcher: Arc<RwLock<FileWatcher>>,
    system: Arc<RwLock<System>>,
    state: Arc<RwLock<ProcessState>>,
}

impl ProcessWatcher {
    pub fn new(
        targets: Vec<Target>,
        log_recorder: LogRecorder,
        file_watcher: Arc<RwLock<FileWatcher>>,
        system: Arc<RwLock<System>>,
    ) -> Self {
        let state = Arc::new(RwLock::new(ProcessState {
            processes: HashMap::new(),
            monitoring: HashMap::new(),
            targets: targets.clone(),
            datasamples_tracker: HashMap::new(),
        }));

        ProcessWatcher {
            ebpf: once_cell::sync::OnceCell::new(),
            log_recorder,
            file_watcher,
            system,
            state,
        }
    }

    /// Updates the list of targets being watched
    pub async fn update_targets(self: &Arc<Self>, targets: Vec<Target>) -> Result<()> {
        let mut state = self.state.write().await;
        state.targets = targets;
        Ok(())
    }

    pub async fn start_ebpf(self: &Arc<Self>) -> Result<()> {
        Arc::clone(self)
            .ebpf
            .get_or_try_init(|| Arc::clone(self).initialize_ebpf())?;
        Ok(())
    }

    fn initialize_ebpf(self: Arc<Self>) -> Result<TracerEbpf, anyhow::Error> {
        let (tx, rx) = mpsc::channel::<Trigger>(100);
        let ebpf = start_processing_events(tx.clone())?;

        let watcher = Arc::clone(&self);
        tokio::spawn(async move {
            watcher.process_trigger_loop(rx).await;
        });

        Ok(ebpf)
    }

    /// Main loop that processes triggers from eBPF
    async fn process_trigger_loop(self: &Arc<Self>, mut rx: mpsc::Receiver<Trigger>) {
        let mut buffer: Vec<Trigger> = Vec::with_capacity(100);

        loop {
            buffer.clear();
            debug!("Ready to receive triggers");

            while rx.recv_many(&mut buffer, 100).await > 0 {
                let triggers = std::mem::take(&mut buffer);
                debug!("Received {:?}", triggers);
                if let Err(e) = self.process_triggers(triggers).await {
                    error!("Failed to process triggers: {}", e);
                }
            }
        }
    }

    /// Processes a batch of triggers, separating start and finish events
    async fn process_triggers(self: &Arc<ProcessWatcher>, triggers: Vec<Trigger>) -> Result<()> {
        let mut start_triggers: Vec<ProcessTrigger> = vec![];
        let mut finish_triggers: Vec<FinishTrigger> = vec![];

        // Separate start and finish triggers
        for trigger in triggers.into_iter() {
            match trigger {
                Trigger::Start(proc) => start_triggers.push(proc),
                Trigger::Finish(proc) => finish_triggers.push(proc),
            }
        }

        // Process finish triggers first
        if !finish_triggers.is_empty() {
            debug!("Processing {} finishing processes", finish_triggers.len());
            self.handle_process_terminations(finish_triggers).await?;
        }

        // Then process start triggers
        if !start_triggers.is_empty() {
            debug!("Processing {} creating processes", start_triggers.len());
            self.handle_process_starts(start_triggers).await?;
        }

        Ok(())
    }

    async fn remove_processes_from_state(
        self: &Arc<Self>,
        triggers: &[FinishTrigger],
    ) -> Result<()> {
        let mut state = self.state.write().await;
        for trigger in triggers.iter() {
            state.processes.remove(&trigger.pid);
        }
        Ok(())
    }

    async fn handle_process_terminations(
        self: &Arc<Self>,
        triggers: Vec<FinishTrigger>,
    ) -> Result<()> {
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
        self: &Arc<Self>,
        start_trigger: &ProcessTrigger,
        finish_trigger: &FinishTrigger,
    ) -> Result<()> {
        let duration_sec = (finish_trigger.finished_at - start_trigger.started_at)
            .num_seconds()
            .try_into()
            .unwrap_or(0);

        let properties = CompletedProcess {
            tool_name: start_trigger.comm.clone(),
            tool_pid: start_trigger.pid.to_string(),
            duration_sec,
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

    async fn handle_process_starts(
        self: &Arc<ProcessWatcher>,
        triggers: Vec<ProcessTrigger>,
    ) -> Result<()> {
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
        self: &Arc<ProcessWatcher>,
        triggers: Vec<ProcessTrigger>,
    ) -> Result<HashMap<Target, HashSet<ProcessTrigger>>> {
        // Store all triggers in the state
        {
            let mut state = self.state.write().await;
            for trigger in triggers.iter() {
                state.processes.insert(trigger.pid, trigger.clone());
            }
        }

        self.find_matching_processes(triggers).await
    }

    /// Refreshes system information for the specified PIDs
    ///
    /// Uses tokio's spawn_blocking to execute the potentially blocking refresh operation
    /// without affecting the async runtime.
    #[tracing::instrument(skip(self))]
    async fn refresh_system(self: &Arc<ProcessWatcher>, pids: &HashSet<usize>) -> Result<()> {
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

    fn get_matched_target<'a>(
        state: &'a ProcessState,
        process: &ProcessTrigger,
    ) -> Option<&'a Target> {
        for target in state.targets.iter() {
            if target.matches_process(process) {
                return Some(target);
            }
        }

        let eligible_targets_for_parents = state
            .targets
            .iter()
            .filter(|target| !target.should_force_ancestor_to_match())
            .collect_vec();

        if eligible_targets_for_parents.is_empty() {
            return None;
        }

        // here it's tempting to check if the parent is just in the monitoring list. However, we can't do that because
        // parent may be matching but not yet set to be monitoring (e.g. because it just arrived or even is in the same batch)

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

    /// Gets a process and all its parent processes from the state
    ///
    /// Will panic if a cycle is detected in the process hierarchy.
    fn get_process_parents<'a>(
        state: &'a ProcessState,
        process: &'a ProcessTrigger,
    ) -> HashSet<&'a ProcessTrigger> {
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

    async fn find_matching_processes(
        self: &Arc<ProcessWatcher>,
        triggers: Vec<ProcessTrigger>,
    ) -> Result<HashMap<Target, HashSet<ProcessTrigger>>> {
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

    async fn handle_new_process(
        self: &Arc<ProcessWatcher>,
        target: &Target,
        process: &ProcessTrigger,
    ) -> Result<ProcessResult> {
        debug!("Processing pid={}", process.pid);

        let display_name = target
            .get_display_name_object()
            .get_display_name(&process.file_name, process.argv.as_slice());

        let properties = {
            let system = self.system.read().await;

            match system.process(process.pid.into()) {
                Some(system_process) => {
                    self.gather_process_data(system_process, display_name.clone(), true)
                        .await
                }
                None => {
                    debug!("Process({}) wasn't found", process.pid);
                    self.create_short_lived_process_properties(process, display_name.clone())
                }
            }
        };

        if let ProcessProperties::Full(ref full_properties) = properties {
            self.log_datasets_in_process(&process.argv, full_properties)
                .await?;
        }

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
        process: &ProcessTrigger,
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
        self: &Arc<ProcessWatcher>,
        target: &Target,
        process: &ProcessTrigger,
    ) -> Result<ProcessResult> {
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
            self.gather_process_data(system_process, display_name.clone(), false)
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

    pub async fn gather_process_data(
        self: &Arc<ProcessWatcher>,
        proc: &Process,
        display_name: String,
        process_input_files: bool,
    ) -> ProcessProperties {
        debug!("Gathering process data for {}", display_name);

        // Get current time (TODO: use process start time when available)
        let start_time = Utc::now();

        let (container_id, job_id, trace_id) = Self::extract_process_env_vars(proc);

        let working_directory = proc.cwd().map(|p| p.to_string_lossy().to_string());

        let input_files = if process_input_files {
            self.extract_input_files(proc).await
        } else {
            None
        };

        ProcessProperties::Full(Box::new(FullProcessProperties {
            tool_name: display_name,
            tool_pid: proc.pid().as_u32().to_string(),
            tool_parent_pid: proc.parent().unwrap_or(0.into()).to_string(),
            tool_binary_path: proc
                .exe()
                .unwrap_or_else(|| Path::new(""))
                .as_os_str()
                .to_str()
                .unwrap_or("")
                .to_string(),
            tool_cmd: proc.cmd().join(" "),
            start_timestamp: start_time.to_rfc3339(),
            process_cpu_utilization: proc.cpu_usage(),
            process_run_time: proc.run_time(),
            process_disk_usage_read_total: proc.disk_usage().total_read_bytes,
            process_disk_usage_write_total: proc.disk_usage().total_written_bytes,
            process_disk_usage_read_last_interval: proc.disk_usage().read_bytes,
            process_disk_usage_write_last_interval: proc.disk_usage().written_bytes,
            process_memory_usage: proc.memory(),
            process_memory_virtual: proc.virtual_memory(),
            process_status: process_status_to_string(&proc.status()),
            input_files,
            container_id,
            job_id,
            working_directory,
            trace_id,
        }))
    }

    async fn extract_input_files(&self, proc: &Process) -> Option<Vec<InputFile>> {
        let mut files = vec![];
        let cmd_arguments = proc.cmd();
        let mut arguments_to_check = Vec::new();

        // Filter arguments to check for files
        for arg in cmd_arguments {
            // Skip flags
            if arg.starts_with('-') {
                continue;
            }

            // Add the original argument
            arguments_to_check.push(arg.clone());

            // Extract values from key-value arguments as additional candidates
            if arg.contains('=') {
                let split: Vec<&str> = arg.split('=').collect();
                if split.len() > 1 {
                    arguments_to_check.push(split[1].to_string());
                }
            }
        }

        // Check if arguments match known files
        let watcher = self.file_watcher.read().await;
        for arg in &arguments_to_check {
            if let Some((path, file_info)) = watcher.get_file_by_path_suffix(arg) {
                files.push(InputFile {
                    file_name: file_info.name.clone(),
                    file_size: file_info.size,
                    file_path: path.clone(),
                    file_directory: file_info.directory.clone(),
                    file_updated_at_timestamp: file_info.last_update.to_rfc3339(),
                });
            }
        }

        if files.is_empty() {
            None
        } else {
            Some(files)
        }
    }

    /// Extracts environment variables related to containerization, jobs, and tracing
    fn extract_process_env_vars(
        proc: &Process,
    ) -> (Option<String>, Option<String>, Option<String>) {
        let mut container_id = None;
        let mut job_id = None;
        let mut trace_id = None;

        // Try to read environment variables
        for env_var in proc.environ() {
            if let Some((key, value)) = env_var.split_once('=') {
                match key {
                    "AWS_BATCH_JOB_ID" => job_id = Some(value.to_string()),
                    "HOSTNAME" => container_id = Some(value.to_string()),
                    "TRACER_TRACE_ID" => trace_id = Some(value.to_string()),
                    _ => continue,
                }
            }
        }

        (container_id, job_id, trace_id)
    }

    /// Builds dataset properties by tracking dataset files used in the process
    ///
    /// Returns dataset properties with information about tracked datasets for the given trace ID
    async fn build_dataset_properties(
        self: &Arc<Self>,
        cmd: &[String],
        trace_id: Option<String>,
    ) -> DataSetsProcessed {
        let trace_key = trace_id.clone().unwrap_or_default();
        let mut state = self.state.write().await;

        // Find and track datasets in command arguments
        for arg in cmd.iter() {
            if DATA_SAMPLES_EXT.iter().any(|ext| arg.ends_with(ext)) {
                state
                    .datasamples_tracker
                    .entry(trace_key.clone())
                    .or_default()
                    .insert(arg.clone());
            }
        }

        // Get the datasets for the current trace
        let datasets = state
            .datasamples_tracker
            .get(&trace_key)
            .map(|set| set.iter().cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default();

        // Get total datasets count
        let total = state
            .datasamples_tracker
            .get(&trace_key)
            .map_or(0, |set| set.len() as u64);

        // Create and return the dataset properties
        DataSetsProcessed {
            datasets,
            total,
            trace_id,
        }
    }

    /// Logs dataset information for a process
    async fn log_datasets_in_process(
        self: &Arc<Self>,
        cmd: &[String],
        properties: &FullProcessProperties,
    ) -> Result<()> {
        let dataset_properties = self
            .build_dataset_properties(cmd, properties.trace_id.clone())
            .await;

        self.log_recorder
            .log(
                TracerProcessStatus::DataSamplesEvent,
                format!("[{}] Samples Processed So Far", Utc::now()),
                Some(EventAttributes::ProcessDatasetStats(dataset_properties)),
                None,
            )
            .await
    }

    /// Polls and updates metrics for all monitored processes
    pub async fn poll_process_metrics(self: &Arc<Self>) -> Result<()> {
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
    async fn update_all_processes(self: &Arc<ProcessWatcher>) -> Result<()> {
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
