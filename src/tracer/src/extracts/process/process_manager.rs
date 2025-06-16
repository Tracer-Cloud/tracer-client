use crate::common::recorder::LogRecorder;
use crate::common::target_process::target_process_manager::TargetManager;
use crate::common::target_process::{Target, TargetMatchable};
use crate::common::types::event::attributes::process::CompletedProcess;
use crate::common::types::event::attributes::EventAttributes;
use crate::common::types::event::ProcessStatus as TracerProcessStatus;
use crate::extracts::process::extract_process_data::ExtractProcessData;
use crate::extracts::process::process_utils::create_short_lived_process_properties;
use crate::extracts::process::types::process_result::ProcessResult;
use crate::extracts::process::types::process_state::ProcessState;
use chrono::Utc;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sysinfo::{Pid, ProcessRefreshKind, System};
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio::task::JoinHandle;
use tracer_ebpf::ebpf_trigger::{
    ExitReason, OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger,
};
use tracing::{debug, error};

pub struct ProcessManager {
    state: Arc<RwLock<ProcessState>>,
    log_recorder: LogRecorder,
    system: Arc<RwLock<System>>,
}

impl ProcessManager {
    pub fn new(target_manager: TargetManager, log_recorder: LogRecorder) -> Self {
        let state = Arc::new(RwLock::new(ProcessState::new(target_manager)));

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
        state.set_ebpf_task(task);
    }

    /// Updates the list of targets being watched
    pub async fn update_targets(&self, targets: Vec<Target>) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        state.update_targets(targets);
        Ok(())
    }

    /// Enriches finish triggers with OOM reason if they were OOM victims
    pub async fn handle_out_of_memory_terminations(
        &self,
        finish_triggers: &mut [ProcessEndTrigger],
    ) {
        let mut state = self.state.write().await;

        for finish in finish_triggers.iter_mut() {
            if state.remove_out_of_memory_victim(&finish.pid).is_some() {
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
            let processes = state.get_processes();
            let is_related =
                processes.contains_key(&oom.pid) || processes.values().any(|p| p.ppid == oom.pid);

            if is_related {
                debug!("Tracking OOM for relevant pid {}", oom.pid);
                victims.insert(oom.pid, oom.clone());
                state.insert_out_of_memory_victim(oom.pid, oom);
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
            state.remove_process(&trigger.pid);
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

    /// Handles newly started processes by filtering, gathering data, and setting up monitoring
    ///
    /// This function:
    /// 1. Filters processes to find those matching our target criteria
    /// 2. Refreshes system data for matched processes
    /// 3. Extracts and logs data for each process
    /// 4. Updates the monitoring state to track these processes
    pub async fn handle_process_starts(
        &self,
        triggers: Vec<ProcessStartTrigger>,
    ) -> anyhow::Result<()> {
        let trigger_count = triggers.len();
        debug!("Processing {} process starts", trigger_count);

        // Find processes we're interested in based on targets
        let filtered_target_processes = self.filter_processes_of_interest(triggers).await?;
        let matched_count = filtered_target_processes.len();
        
        debug!("After filtering, matched {} processes out of {}", matched_count, trigger_count);

        if filtered_target_processes.is_empty() {
            return Ok(());
        }

        // Collect all PIDs that need system data refreshed
        let pids_to_refresh = self.collect_pids_to_refresh(&filtered_target_processes);
        
        // Refresh system data for these processes
        self.refresh_system(&pids_to_refresh).await?;

        // Process each matched process
        self.process_matched_processes(&filtered_target_processes).await?;

        // Update monitoring state with new processes
        self.update_monitoring_state(filtered_target_processes).await?;

        Ok(())
    }

    /// Collects all PIDs from the filtered target processes map
    fn collect_pids_to_refresh(&self, filtered_target_processes: &HashMap<Target, HashSet<ProcessStartTrigger>>) -> HashSet<usize> {
        filtered_target_processes
            .values()
            .flat_map(|procs| procs.iter().map(|p| p.pid))
            .collect()
    }

    /// Processes each matched process by extracting and logging its data
    async fn process_matched_processes(
        &self, 
        filtered_target_processes: &HashMap<Target, HashSet<ProcessStartTrigger>>
    ) -> anyhow::Result<()> {
        for (target, processes) in filtered_target_processes.iter() {
            for process in processes.iter() {
                self.handle_new_process(target, process).await?;
            }
        }
        Ok(())
    }

    /// Updates the monitoring state with new processes
    async fn update_monitoring_state(
        &self,
        filtered_target_processes: HashMap<Target, HashSet<ProcessStartTrigger>>
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        state.update_monitoring(filtered_target_processes);
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
                state.insert_process(trigger.pid, trigger.clone());
            }
        }

        // Get PIDs of processes already being monitored
        let state = self.state.read().await;
        let already_monitored_pids = state.get_monitored_processes_pids();

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
                        let mut parents = state.get_process_hierarchy(proc);
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
        if let Some(target) = state.get_target_manager().get_target_match(process) {
            return Some(target);
        }

        let eligible_targets_for_parents = state
            .get_target_manager()
            .targets
            .iter()
            .filter(|target| !target.should_force_ancestor_to_match())
            .collect_vec();

        if eligible_targets_for_parents.is_empty() {
            return None;
        }

        // Here it's tempting to check if the parent is just in the monitoring list. However, we can't do that because
        // parent may be matching but not yet set to be monitoring (e.g., because it just arrived or even is in the same batch)

        let parents = state.get_process_parents(process);
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
                    create_short_lived_process_properties(process, display_name.clone())
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
            debug!(
                "Refreshing data for {} processes",
                state.get_monitoring().len()
            );

            if state.get_monitoring().is_empty() {
                debug!("No processes to monitor, skipping poll");
                return Ok(());
            }

            state
                .get_monitoring()
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
    pub async fn get_n_monitored_processes(&self, n: usize) -> HashSet<String> {
        self.state
            .read()
            .await
            .get_monitoring()
            .iter()
            .flat_map(|(_, processes)| processes.iter().map(|p| p.comm.clone()))
            .take(n)
            .collect()
    }

    /// Returns the total number of processes being monitored
    pub async fn get_number_of_monitored_processes(&self) -> usize {
        self.state
            .read()
            .await
            .get_monitoring()
            .values()
            .map(|processes| processes.len())
            .sum()
    }

    /// Updates all monitored processes with fresh data
    #[tracing::instrument(skip(self))]
    async fn update_all_processes(&self) -> anyhow::Result<()> {
        for (target, procs) in self.state.read().await.get_monitoring().iter() {
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
    use crate::common::target_process::target_matching::TargetMatch;
    use crate::common::target_process::targets_list::TARGETS;
    use crate::common::types::current_run::{PipelineMetadata, Run};
    use crate::common::types::pipeline_tags::PipelineTags;
    use chrono::DateTime;
    use rstest::rstest;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    // Helper function to create a process trigger with specified properties
    fn create_process_start_trigger(
        pid: usize,
        ppid: usize,
        comm: &str,
        args: Vec<&str>,
        file_name: &str,
    ) -> ProcessStartTrigger {
        ProcessStartTrigger {
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

    #[tokio::test]
    async fn test_find_matching_processes_direct_match() {
        // Create a target and set up the process manager
        let target = Target::new(TargetMatch::ProcessName("test_process".to_string()));
        let target_manager = TargetManager::new(vec![target.clone()], vec![]);
        let log_recorder = create_mock_log_recorder();

        let process_manager = ProcessManager::new(target_manager, log_recorder);

        // Create a process that directly matches the target
        let process = create_process_start_trigger(
            100,
            1,
            "test_process",
            vec!["test_process", "--arg1", "value1"],
            "/usr/bin/test_process",
        );

        // Test the function
        let result = process_manager
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
        let target_manager = TargetManager::new(vec![target.clone()], vec![]);
        let log_recorder = create_mock_log_recorder();

        let process_manager = ProcessManager::new(target_manager, log_recorder);

        // Create a process that doesn't match any target
        let process = create_process_start_trigger(
            100,
            1,
            "non_matching_process",
            vec!["non_matching_process", "--arg1", "value1"],
            "/usr/bin/non_matching_process",
        );

        // Test the function
        let result = process_manager
            .find_matching_processes(vec![process])
            .await
            .unwrap();

        // Assert no processes were matched
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_find_matching_processes_parent_match_with_force_ancestor_false() {
        // Create a target that matches a parent process and has force_ancestor_to_match=false
        let target = Target::new(TargetMatch::ProcessName("parent_process".to_string()))
            .set_force_ancestor_to_match(false);

        // Create a parent process
        let parent_process = create_process_start_trigger(
            50,
            1,
            "parent_process",
            vec!["parent_process"],
            "/usr/bin/parent_process",
        );

        // Create a child process that doesn't match any target
        let child_process = create_process_start_trigger(
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
        let target_manager = TargetManager::new(vec![target.clone()], vec![]);
        let log_recorder = create_mock_log_recorder();

        let process_manager = ProcessManager::new(target_manager, log_recorder);
        process_manager
            .get_state_mut()
            .await
            .set_processes(processes);

        // Test with the child process
        let result = process_manager
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
        // Create a target that matches a parent process but has force_ancestor_to_match=true
        let target = Target::new(TargetMatch::ProcessName("parent_process".to_string()));
        // force_ancestor_to_match is true by default

        // Create a parent process
        let parent_process = create_process_start_trigger(
            50,
            1,
            "parent_process",
            vec!["parent_process"],
            "/usr/bin/parent_process",
        );

        // Create a child process that doesn't match any target
        let child_process = create_process_start_trigger(
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
        let target_manager = TargetManager::new(vec![target], vec![]);
        let log_recorder = create_mock_log_recorder();

        let process_manager = ProcessManager::new(target_manager, log_recorder);
        let mut _state_processes = process_manager.get_state_mut().await.get_processes();

        _state_processes = &processes;

        // Test with the child process
        let result = process_manager
            .find_matching_processes(vec![child_process])
            .await
            .unwrap();

        // Assert the child process was NOT matched to the target because force_ancestor_to_match is true
        assert_eq!(result.len(), 0);
    }

    #[rstest]
    #[case::excluded_bash(
        create_process_start_trigger(
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
        create_process_start_trigger(
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
    create_process_start_trigger(
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
        create_process_start_trigger(
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
        create_process_start_trigger(
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
        #[case] process: ProcessStartTrigger,
        #[case] expected_count: usize,
        #[case] msg: &str,
    ) {
        let target_manager = TargetManager::new(TARGETS.to_vec(), vec![]);
        let log_recorder = create_mock_log_recorder();

        let process_manager = ProcessManager::new(target_manager, log_recorder);

        let result = process_manager
            .find_matching_processes(vec![process])
            .await
            .unwrap();

        assert_eq!(result.len(), expected_count, "{}", msg);
    }

    #[rstest]
    #[case::command_script(
        create_process_start_trigger(
        202,
        1,
        "nextflow",
        vec!["bash", "/nextflow_work/01/5152d22e188cfc22ef4c4c6cd9fc9e/.command.sh"],
        "/usr/bin/bash"
    )
)]
    #[case::command_dot_run(
        create_process_start_trigger(
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
    async fn test_nextflow_wrapped_scripts(#[case] process: ProcessStartTrigger) {
        let target_manager = TargetManager::new(TARGETS.to_vec(), vec![]);
        let log_recorder = create_mock_log_recorder();

        let process_manager = ProcessManager::new(target_manager, log_recorder);
        let result = process_manager
            .find_matching_processes(vec![process])
            .await
            .unwrap();

        assert_eq!(
            result.len(),
            0,
            "Expected no matches for wrapped nextflow script"
        );
    }
}
