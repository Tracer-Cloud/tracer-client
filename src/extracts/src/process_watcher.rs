use chrono::DateTime;
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
use tracer_common::target_process::manager::TargetManager;
use tracer_common::target_process::{Target, TargetMatchable};
use tracer_common::types::event::attributes::process::{
    CompletedProcess, DataSetsProcessed, FullProcessProperties, InputFile, ProcessProperties,
    ShortProcessProperties,
};
use tracer_common::types::event::attributes::EventAttributes;
use tracer_common::types::trigger::{FinishTrigger, ProcessTrigger, Trigger};
use tracer_ebpf::binding::start_processing_events;
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
    target_manager: TargetManager,
    // Store task handle to ensure it stays alive
    ebpf_task: Option<tokio::task::JoinHandle<()>>,
}

/// Watches system processes and records events related to them
pub struct ProcessWatcher {
    ebpf: once_cell::sync::OnceCell<()>, // not tokio, because ebpf initialisation is sync
    log_recorder: LogRecorder,
    file_watcher: Arc<RwLock<FileWatcher>>,
    system: Arc<RwLock<System>>,
    state: Arc<RwLock<ProcessState>>,
}

impl ProcessWatcher {
    pub fn new(
        target_manager: TargetManager,
        log_recorder: LogRecorder,
        file_watcher: Arc<RwLock<FileWatcher>>,
        system: Arc<RwLock<System>>,
    ) -> Self {
        let state = Arc::new(RwLock::new(ProcessState {
            processes: HashMap::new(),
            monitoring: HashMap::new(),
            target_manager,
            datasamples_tracker: HashMap::new(),
            ebpf_task: None,
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
        state.target_manager.targets = targets;
        Ok(())
    }

    pub async fn start_ebpf(self: &Arc<Self>) -> Result<()> {
        Arc::clone(self)
            .ebpf
            .get_or_try_init(|| Arc::clone(self).initialize_ebpf())?;
        Ok(())
    }

    fn initialize_ebpf(self: Arc<Self>) -> Result<(), anyhow::Error> {
        // Use unbounded channel for cross-runtime compatibility
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();

        // Start the eBPF event processing
        start_processing_events(tx)?;

        // Start the event processing loop
        let watcher = Arc::clone(&self);
        let task = tokio::spawn(async move {
            if let Err(e) = watcher.process_trigger_loop(rx).await {
                error!("process_trigger_loop failed: {:?}", e);
            }
        });

        // Store the task handle in the state
        match tokio::runtime::Handle::try_current() {
            Ok(_) => {
                tokio::spawn(async move {
                    let mut state = self.state.write().await;
                    state.ebpf_task = Some(task);
                });
            }
            Err(_) => {
                // Not in a tokio runtime, can't store the task handle
            }
        }

        Ok(())
    }

    /// Main loop that processes triggers from eBPF
    async fn process_trigger_loop(
        self: &Arc<Self>,
        mut rx: mpsc::UnboundedReceiver<Trigger>,
    ) -> Result<()> {
        let mut buffer: Vec<Trigger> = Vec::with_capacity(100);

        loop {
            buffer.clear();
            debug!("Ready to receive triggers");

            // Since UnboundedReceiver doesn't have recv_many, we need to use a different approach
            // Try to receive a single event with timeout to avoid blocking forever
            match tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await {
                Ok(Some(event)) => {
                    buffer.push(event);

                    // Try to receive more events non-blockingly (up to 99 more)
                    while let Ok(Some(event)) =
                        tokio::time::timeout(std::time::Duration::from_millis(10), rx.recv()).await
                    {
                        buffer.push(event);
                        if buffer.len() >= 100 {
                            break;
                        }
                    }

                    // Process all events
                    let triggers = std::mem::take(&mut buffer);
                    println!("Received {:?}", triggers);

                    if let Err(e) = self.process_triggers(triggers).await {
                        error!("Failed to process triggers: {}", e);
                    }
                }
                Ok(None) => {
                    error!("Event channel closed, exiting process loop");
                    return Ok(());
                }
                Err(_) => {
                    // Timeout occurred, just continue the loop
                    continue;
                }
            }
        }
    }

    /// Processes a batch of triggers, separating start and finish events
    pub async fn process_triggers(
        self: &Arc<ProcessWatcher>,
        triggers: Vec<Trigger>,
    ) -> Result<()> {
        let mut start_triggers: Vec<ProcessTrigger> = vec![];
        let mut finish_triggers: Vec<FinishTrigger> = vec![];

        // Add debug logging
        debug!("ProcessWatcher: processing {} triggers", triggers.len());

        // Separate start and finish triggers
        for trigger in triggers.into_iter() {
            match trigger {
                Trigger::Start(proc) => {
                    debug!(
                        "ProcessWatcher: received START trigger pid={}, cmd={}",
                        proc.pid, proc.comm
                    );
                    start_triggers.push(proc);
                }
                Trigger::Finish(proc) => {
                    debug!("ProcessWatcher: received FINISH trigger pid={}", proc.pid);
                    finish_triggers.push(proc);
                }
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
                        let mut parents = Self::get_process_hierarchy(&state, proc);
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

    /// Gets a process and all its parent processes from the state
    ///
    /// Will panic if a cycle is detected in the process hierarchy.
    fn get_process_hierarchy(
        state: &ProcessState,
        process: ProcessTrigger,
    ) -> HashSet<ProcessTrigger> {
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

    pub async fn find_matching_processes(
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

    fn get_matched_target<'a>(
        state: &'a ProcessState,
        process: &ProcessTrigger,
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
                    self.gather_process_data(
                        system_process,
                        display_name.clone(),
                        true,
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
            self.gather_process_data(
                system_process,
                display_name.clone(),
                false,
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

    pub async fn gather_process_data(
        self: &Arc<ProcessWatcher>,
        proc: &Process,
        display_name: String,
        process_input_files: bool,
        process_start_time: DateTime<Utc>,
    ) -> ProcessProperties {
        debug!("Gathering process data for {}", display_name);

        let (container_id, job_id, trace_id) = Self::extract_process_env_vars(proc);

        let working_directory = proc.cwd().map(|p| p.to_string_lossy().to_string());

        let input_files = if process_input_files {
            self.extract_input_files(proc).await
        } else {
            None
        };

        let process_run_time = (Utc::now() - process_start_time).num_milliseconds().max(0) as u64;

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
            start_timestamp: process_start_time.to_rfc3339(),
            process_cpu_utilization: proc.cpu_usage(),
            process_run_time, // time in milliseconds
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use rstest::rstest;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;
    use tracer_common::target_process::target_matching::{CommandContainsStruct, TargetMatch};
    use tracer_common::target_process::targets_list::TARGETS;
    use tracer_common::types::current_run::{PipelineMetadata, Run};
    use tracer_common::types::pipeline_tags::PipelineTags;

    // Helper function to create a process trigger with specified properties
    fn create_process_trigger(
        pid: usize,
        ppid: usize,
        comm: &str,
        args: Vec<&str>,
        file_name: &str,
    ) -> ProcessTrigger {
        ProcessTrigger {
            pid,
            ppid,
            comm: comm.to_string(),
            argv: args.iter().map(|s| s.to_string()).collect(),
            file_name: file_name.to_string(),
            started_at: DateTime::parse_from_rfc3339("2025-05-07T00:00:00Z")
                .unwrap()
                .into(),
        }
    }

    // Helper function to create a mock LogRecorder
    fn create_mock_log_recorder() -> LogRecorder {
        let pipeline = PipelineMetadata {
            pipeline_name: "test_pipeline".to_string(),
            run: Some(Run::new("test_run".to_string(), "test-id-123".to_string())),
            tags: PipelineTags::default(),
        };
        let pipeline_arc = Arc::new(RwLock::new(pipeline));
        let (tx, _rx) = mpsc::channel(10);
        LogRecorder::new(pipeline_arc, tx)
    }

    // Helper function to create a mock FileWatcher
    fn create_mock_file_watcher() -> Arc<RwLock<FileWatcher>> {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        Arc::new(RwLock::new(FileWatcher::new(temp_dir)))
    }

    // Helper function to set up a process watcher with specified targets and processes
    fn setup_process_watcher(
        target_manager: TargetManager,
        processes: HashMap<usize, ProcessTrigger>,
    ) -> Arc<ProcessWatcher> {
        let state = ProcessState {
            processes,
            monitoring: HashMap::new(),
            target_manager,
            datasamples_tracker: HashMap::new(),
            ebpf_task: None,
        };

        let log_recorder = create_mock_log_recorder();
        let system = Arc::new(RwLock::new(System::new_all()));
        let file_watcher = create_mock_file_watcher();
        let state = Arc::new(RwLock::new(state));

        Arc::new(ProcessWatcher {
            ebpf: once_cell::sync::OnceCell::new(),
            log_recorder,
            file_watcher,
            system,
            state,
        })
    }

    #[tokio::test]
    async fn test_find_matching_processes_direct_match() {
        // Create a target and set up the watcher
        let target = Target::new(TargetMatch::ProcessName("test_process".to_string()));
        let mgr = TargetManager::new(vec![target.clone()], vec![]);
        let watcher = setup_process_watcher(mgr, HashMap::new());

        // Create a process that directly matches the target
        let process = create_process_trigger(
            100,
            1,
            "test_process",
            vec!["test_process", "--arg1", "value1"],
            "/usr/bin/test_process",
        );

        // Test the function
        let result = watcher
            .find_matching_processes(vec![process])
            .await
            .unwrap();

        // Assert the process was matched to the target
        assert_eq!(result.len(), 1);
        assert!(result.contains_key(&target));
    }

    #[tokio::test]
    async fn test_find_matching_processes_no_match() {
        // Create a target and set up the watcher
        let target = Target::new(TargetMatch::ProcessName("test_process".to_string()));
        let mgr = TargetManager::new(vec![target.clone()], vec![]);
        let watcher = setup_process_watcher(mgr, HashMap::new());

        // Create a process that doesn't match any target
        let process = create_process_trigger(
            100,
            1,
            "non_matching_process",
            vec!["non_matching_process", "--arg1", "value1"],
            "/usr/bin/non_matching_process",
        );

        // Test the function
        let result = watcher
            .find_matching_processes(vec![process])
            .await
            .unwrap();

        // Assert no processes were matched
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_find_matching_processes_parent_match_with_force_ancestor_false() {
        // Create a target that matches parent process and has force_ancestor_to_match=false
        let target = Target::new(TargetMatch::ProcessName("parent_process".to_string()))
            .set_force_ancestor_to_match(false);

        // Create a parent process
        let parent_process = create_process_trigger(
            50,
            1,
            "parent_process",
            vec!["parent_process"],
            "/usr/bin/parent_process",
        );

        // Create a child process that doesn't match any target
        let child_process = create_process_trigger(
            100,
            50, // Parent PID is 50
            "child_process",
            vec!["child_process"],
            "/usr/bin/child_process",
        );

        // Create the initial state with the parent process already in it
        let mut processes = HashMap::new();
        processes.insert(parent_process.pid, parent_process);

        // Set up the watcher with these processes and target
        let mgr = TargetManager::new(vec![target.clone()], vec![]);
        let watcher = setup_process_watcher(mgr, processes);

        // Test with the child process
        let result = watcher
            .find_matching_processes(vec![child_process.clone()])
            .await
            .unwrap();

        // Assert the child process was matched to the target because its parent matches
        // and force_ancestor_to_match is false
        assert_eq!(result.len(), 1);
        assert!(result.contains_key(&target));

        // Also verify the child process is the one that was matched
        let matched_processes = result.get(&target).unwrap();
        assert_eq!(matched_processes.len(), 1);
        assert!(matched_processes.contains(&child_process));
    }

    #[tokio::test]
    async fn test_find_matching_processes_parent_match_with_force_ancestor_true() {
        // Create a target that matches parent process but has force_ancestor_to_match=true
        let target = Target::new(TargetMatch::ProcessName("parent_process".to_string()));
        // force_ancestor_to_match is true by default

        // Create a parent process
        let parent_process = create_process_trigger(
            50,
            1,
            "parent_process",
            vec!["parent_process"],
            "/usr/bin/parent_process",
        );

        // Create a child process that doesn't match any target
        let child_process = create_process_trigger(
            100,
            50, // Parent PID is 50
            "child_process",
            vec!["child_process"],
            "/usr/bin/child_process",
        );

        // Create the initial state with the parent process already in it
        let mut processes = HashMap::new();
        processes.insert(parent_process.pid, parent_process);

        // Set up the watcher with these processes and target
        let mgr = TargetManager::new(vec![target], vec![]);
        let watcher = setup_process_watcher(mgr, processes);

        // Test with the child process
        let result = watcher
            .find_matching_processes(vec![child_process])
            .await
            .unwrap();

        // Assert the child process was NOT matched to the target because force_ancestor_to_match is true
        assert_eq!(result.len(), 0);
    }

    #[rstest]
    #[case::excluded_bash(
    create_process_trigger(
        100,
        1,
        "bash",
        vec!["/opt/conda/bin/bash", "script.sh"],
        "/opt/conda/bin/bash"
    ),
    0,
    "Should exclude bash in /opt/conda/bin due to filter_out exception list"
)]
    #[case::included_foo(
    create_process_trigger(
        101,
        1,
        "foo",
        vec!["/opt/conda/bin/foo", "--version"],
        "/opt/conda/bin/foo"
    ),
    1,
    "Should match /opt/conda/bin/foo as it's not in filter_out exception list"
)]
    #[case::unmatched_usr_bash(
    create_process_trigger(
        102,
        1,
        "bash",
        vec!["/usr/bin/bash", "other.sh"],
        "/usr/bin/bash"
    ),
    0,
    "Should not match bash in /usr/bin since there's no explicit target for it"
)]
    #[case::nextflow_local_conf_command(
    create_process_trigger(
        200,
        1,
        "local.conf",
        vec![
            "bash",
            "-c",
            ". spack/share/spack/setup-env.sh; spack env activate -d .; cd frameworks/nextflow && nextflow -c nextflow-config/local.config run pipelines/nf-core/rnaseq/main.nf -params-file nextflow-config/rnaseq-params.json -profile test"
        ],
        "/usr/bin/bash"
    ),
    0,
    "Should not match local.conf-based bash wrapper"
)]
    #[case::nextflow_wrapper_bash_command(
    create_process_trigger(
        201,
        1,
        "nextflow",
        vec![
            "bash",
            "-c",
            ". spack/share/spack/setup-env.sh; spack env activate -d .; cd frameworks/nextflow && nextflow -c nextflow-config/local.config run pipelines/nf-core/rnaseq/main.nf -params-file nextflow-config/rnaseq-params.json -profile test"
        ],
        "/usr/bin/bash"
    ),
    0,
    "Should not match bash-wrapped nextflow script (known wrapper)"
)]
    #[tokio::test]
    async fn test_match_cases(
        #[case] process: ProcessTrigger,
        #[case] expected_count: usize,
        #[case] msg: &str,
    ) {
        let mgr = TargetManager::new(TARGETS.to_vec(), vec![]);
        let watcher = setup_process_watcher(mgr, HashMap::new());

        let result = watcher
            .find_matching_processes(vec![process])
            .await
            .unwrap();

        assert_eq!(result.len(), expected_count, "{}", msg);
    }

    #[rstest]
    #[case::command_script(
    create_process_trigger(
        202,
        1,
        "nextflow",
        vec!["bash", "/nextflow_work/01/5152d22e188cfc22ef4c4c6cd9fc9e/.command.sh"],
        "/usr/bin/bash"
    )
)]
    #[case::command_dot_run(
    create_process_trigger(
        203,
        1,
        "nextflow",
        vec![
            "/bin/bash",
            "/nextflow_work/01/5152d22e188cfc22ef4c4c6cd9fc9e/.command.run",
            "nxf_trace"
        ],
        "/bin/bash"
    )
)]
    #[tokio::test]
    async fn test_nextflow_wrapped_scripts(#[case] process: ProcessTrigger) {
        let mgr = TargetManager::new(TARGETS.to_vec(), vec![]);
        let watcher = setup_process_watcher(mgr, HashMap::new());
        let result = watcher
            .find_matching_processes(vec![process])
            .await
            .unwrap();

        assert_eq!(
            result.len(),
            0,
            "Expected no matches for wrapped nextflow script"
        );
    }
    fn dummy_process(name: &str, cmd: &str, path: &str) -> ProcessTrigger {
        ProcessTrigger {
            pid: 1,
            ppid: 0,
            comm: name.to_string(),
            argv: cmd.split_whitespace().map(String::from).collect(),
            file_name: path.to_string(),
            started_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_blacklist_excludes_match() {
        let blacklist = vec![Target::new(TargetMatch::CommandContains(
            CommandContainsStruct {
                process_name: None,
                command_content: "spack".to_string(),
            },
        ))];
        let targets = vec![Target::new(TargetMatch::ProcessName("fastqc".to_string()))];

        let mgr = TargetManager::new(targets, blacklist);
        let proc = dummy_process("fastqc", "spack activate && fastqc", "/usr/bin/fastqc");

        assert!(mgr.get_target_match(&proc).is_none());
    }

    #[test]
    fn test_target_match_without_blacklist() {
        let mgr = TargetManager::new(
            vec![Target::new(TargetMatch::ProcessName("fastqc".to_string()))],
            vec![],
        );
        let proc = dummy_process("fastqc", "fastqc file.fq", "/usr/bin/fastqc");
        assert!(mgr.get_target_match(&proc).is_some());
    }
}
