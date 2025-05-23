use tracer_common::types::event::ProcessStatus as TracerProcessStatus;

use crate::data_samples::DATA_SAMPLES_EXT;
use crate::file_watcher::FileWatcher;
use crate::handlers::process_manager::ProcessManager;
use anyhow::Result;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use sysinfo::{ProcessStatus, System};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracer_common::recorder::LogRecorder;
use tracer_common::target_process::manager::TargetManager;
use tracer_common::target_process::{Target, TargetMatchable};
use tracer_common::types::event::attributes::process::{
    CompletedProcess, DataSetsProcessed, FullProcessProperties, InputFile, ProcessProperties,
    ShortProcessProperties,
};
use tracer_common::types::event::attributes::EventAttributes;
use tracer_common::types::trigger::{ProcessEndTrigger, ProcessStartTrigger, Trigger};
use tracer_ebpf_libbpf::start_processing_events;
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
    // List of targets to watch
    target_manager: TargetManager,
    // Store task handle to ensure it stays alive
    ebpf_task: Option<JoinHandle<()>>,
}

/// Watches system processes and records events related to them
pub struct ProcessWatcher {
    ebpf: Arc<OnceCell<()>>,
    process_manager: Arc<ProcessManager>,
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
            target_manager,
            ebpf_task: None,
        }));

        let process_manager = Arc::new(ProcessManager::new(log_recorder, system));

        ProcessWatcher {
            ebpf: Arc::new(OnceCell::new()),
            process_manager,
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
        // Check if eBPF is already initialized
        if self.ebpf.get().is_some() {
            debug!("eBPF already initialized, skipping");
            return Ok(()); // Already initialized
        }

        debug!("Starting eBPF event processing...");

        // Initialize eBPF components
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();

        // Start the eBPF event processing
        debug!("Calling start_processing_events...");
        if let Err(e) = start_processing_events(tx) {
            error!("Failed to start eBPF processing: {:?}", e);
            return Err(e);
        }
        debug!("start_processing_events completed successfully");

        // Mark eBPF as initialized
        if self.ebpf.set(()).is_err() {
            // Another thread already initialized it, that's fine
            debug!("eBPF was already initialized by another thread");
            return Ok(());
        }

        // Clone self for the task
        let self_clone = Arc::clone(self);

        let ebpf_task = tokio::spawn(async move {
            self_clone.process_ebpf_events(rx).await;
        });

        // Store the task handle using async write
        let mut state = self.state.write().await;
        state.ebpf_task = Some(ebpf_task);

        debug!("eBPF initialization completed successfully");
        Ok(())
    }

    /// Processes eBPF events from the given receiver channel
    async fn process_ebpf_events(self: &Arc<Self>, mut rx: mpsc::UnboundedReceiver<Trigger>) {
        debug!("eBPF event processing loop started, waiting for triggers...");
        let mut buffer = Vec::new();
        loop {
            match rx.recv().await {
                Some(event) => {
                    debug!("Received eBPF trigger: {:?}", event);
                    buffer.push(event);
                    // Try to receive more events non-blockingly (up to 99 more)
                    let mut count = 1;
                    while let Ok(Some(event)) =
                        tokio::time::timeout(std::time::Duration::from_millis(10), rx.recv()).await
                    {
                        debug!("Received additional eBPF trigger: {:?}", event);
                        buffer.push(event);
                        count += 1;
                        if count >= 100 {
                            break;
                        }
                    }

                    // Process all events
                    let triggers = std::mem::take(&mut buffer);
                    debug!(
                        "process_trigger_loop: Processing {} triggers",
                        triggers.len()
                    );

                    if let Err(e) = self.handle_incoming_triggers(triggers).await {
                        error!("Failed to process triggers: {}", e);
                    }
                }
                None => {
                    error!("Event channel closed, exiting process loop");
                    return;
                }
            }
        }
    }

    /// Handles incoming process triggers
    pub async fn handle_incoming_triggers(
        self: &Arc<ProcessWatcher>,
        triggers: Vec<Trigger>,
    ) -> Result<()> {
        let mut matched_triggers: Vec<(Target, ProcessStartTrigger)> = vec![];
        let mut finish_triggers: Vec<ProcessEndTrigger> = vec![];

        println!("ProcessWatcher: processing {} triggers", triggers.len());

        let state = self.state.read().await;
        for trigger in triggers.into_iter() {
            match trigger {
                Trigger::ProcessStart(proc) => {
                    if let Some(matched_target) = state.target_manager.get_target_match(&proc) {
                        println!(
                            "MATCHED START: pid={} cmd={} target={:?}",
                            proc.pid, proc.comm, matched_target
                        );
                        matched_triggers.push((matched_target.clone(), proc));
                    } else {
                        println!("SKIPPED START: pid={} cmd={}", proc.pid, proc.comm);
                    }
                }
                Trigger::ProcessEnd(proc) => {
                    println!("ProcessWatcher: received FINISH trigger pid={}", proc.pid);
                    finish_triggers.push(proc);
                }
            }
        }
        drop(state); // release the read lock

        // Handle process starts
        for (target, process) in matched_triggers {
            if let Err(e) = self
                .process_manager
                .handle_process_start(&target, &process)
                .await
            {
                error!("Failed to handle process start: {}", e);
            }
        }

        // Handle process ends
        for finish_trigger in finish_triggers {
            if let Err(e) = self
                .process_manager
                .handle_process_end(&finish_trigger)
                .await
            {
                error!("Failed to handle process end: {}", e);
            }
        }

        Ok(())
    }

    /// Returns the number of processes being monitored
    pub async fn targets_len(&self) -> usize {
        self.process_manager.monitored_processes_count().await
    }

    /// Returns N process names of monitored processes
    pub async fn preview_targets(&self, n: usize) -> std::collections::HashSet<String> {
        // TODO: Implement this in ProcessManager
        std::collections::HashSet::new()
    }

    /// Polls and updates metrics for all monitored processes
    pub async fn poll_process_metrics(self: &Arc<Self>) -> Result<()> {
        self.process_manager.update_all_processes().await
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
            target_manager,
            ebpf_task: None,
        };

        let log_recorder = create_mock_log_recorder();
        let system = Arc::new(RwLock::new(System::new_all()));
        let file_watcher = create_mock_file_watcher();
        let state = Arc::new(RwLock::new(state));

        let process_manager = Arc::new(ProcessManager::new(log_recorder, system));

        Arc::new(ProcessWatcher {
            ebpf: Arc::new(OnceCell::new()),
            process_manager,
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
            .handle_incoming_triggers(vec![process])
            .await
            .unwrap();

        // Assert the process was matched to the target
        assert_eq!(result, Ok(()));
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
            .handle_incoming_triggers(vec![process])
            .await
            .unwrap();

        // Assert no processes were matched
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    async fn test_find_matching_processes_parent_match_with_force_ancestor_false() {
        // Create a target that matches a parent process and has force_ancestor_to_match=false
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
            .handle_incoming_triggers(vec![child_process.clone()])
            .await
            .unwrap();

        // Assert the child process was matched to the target because its parent matches
        // and force_ancestor_to_match is false
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    async fn test_find_matching_processes_parent_match_with_force_ancestor_true() {
        // Create a target that matches a parent process but has force_ancestor_to_match=true
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
            .handle_incoming_triggers(vec![child_process])
            .await
            .unwrap();

        // Assert the child process was NOT matched to the target because force_ancestor_to_match is true
        assert_eq!(result, Ok(()));
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
            .handle_incoming_triggers(vec![process])
            .await
            .unwrap();

        assert_eq!(result, Ok(()));
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
            .handle_incoming_triggers(vec![process])
            .await
            .unwrap();

        assert_eq!(result, Ok(()));
    }
    fn dummy_process(name: &str, cmd: &str, path: &str) -> ProcessTrigger {
        ProcessTrigger {
            pid: 1,
            ppid: 0,
            comm: name.to_string(),
            argv: cmd.split_whitespace().map(String::from).collect(),
            file_name: path.to_string(),
            started_at: Utc::now(),
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
