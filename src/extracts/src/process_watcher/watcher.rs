use crate::file_watcher::FileWatcher;
use crate::process_watcher::process_manager::ProcessManager;
use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessStatus, System};
use tokio::sync::{mpsc, RwLock};
use tracer_common::recorder::LogRecorder;
use tracer_common::target_process::manager::TargetManager;
use tracer_common::target_process::Target;
use tracer_common::types::ebpf_trigger::{
    OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger, Trigger,
};
use tracer_common::types::event::attributes::process::{
    CompletedProcess, DataSetsProcessed, FullProcessProperties, InputFile, ProcessProperties,
    ShortProcessProperties,
};
use tracer_common::types::event::attributes::EventAttributes;
use tracer_ebpf::binding::start_processing_events;
use tracing::{debug, error};

/// Watches system processes and records events related to them
pub struct ProcessWatcher {
    ebpf: once_cell::sync::OnceCell<()>, // not tokio, because ebpf initialisation is sync
    log_recorder: LogRecorder,
    file_watcher: Arc<RwLock<FileWatcher>>,
    system: Arc<RwLock<System>>,
    process_manager: Arc<RwLock<ProcessManager>>,
}

impl ProcessWatcher {
    pub fn new(
        target_manager: TargetManager,
        log_recorder: LogRecorder,
        file_watcher: Arc<RwLock<FileWatcher>>,
        system: Arc<RwLock<System>>,
    ) -> Self {
        // instantiate the process manager
        let process_manager = Arc::new(RwLock::new(ProcessManager::new(
            target_manager.clone(),
            log_recorder.clone(),
        )));

        ProcessWatcher {
            ebpf: once_cell::sync::OnceCell::new(),
            log_recorder,
            file_watcher,
            system,
            process_manager,
        }
    }

    pub async fn update_targets(self: &Arc<Self>, targets: Vec<Target>) -> anyhow::Result<()> {
        self.process_manager
            .write()
            .await
            .update_targets(targets)
            .await?;
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
                    let mut process_manager = self.process_manager.write().await;
                    let mut state = process_manager.get_state_mut().await;
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

    /// Processes a batch of triggers, separating start, finish, and OOM events
    pub async fn process_triggers(self: &Arc<Self>, triggers: Vec<Trigger>) -> Result<()> {
        let mut process_start_triggers: Vec<ProcessStartTrigger> = vec![];
        let mut process_end_triggers: Vec<ProcessEndTrigger> = vec![];
        let mut out_of_memory_triggers: Vec<OutOfMemoryTrigger> = vec![];

        debug!("ProcessWatcher: processing {} triggers", triggers.len());

        for trigger in triggers.into_iter() {
            match trigger {
                Trigger::ProcessStart(process_started) => {
                    debug!(
                        "ProcessWatcher: received START trigger pid={}, cmd={}",
                        process_started.pid, process_started.comm
                    );
                    process_start_triggers.push(process_started);
                }
                Trigger::ProcessEnd(process_end) => {
                    debug!(
                        "ProcessWatcher: received FINISH trigger pid={}",
                        process_end.pid
                    );
                    process_end_triggers.push(process_end);
                }
                Trigger::OutOfMemory(out_of_memory) => {
                    debug!("OOM trigger pid={}", out_of_memory.pid);
                    out_of_memory_triggers.push(out_of_memory);
                }
            }
        }

        // Process omm triggers first
        if !out_of_memory_triggers.is_empty() {
            debug!("Processing {} oom processes", out_of_memory_triggers.len());
            self.process_manager
                .write()
                .await
                .handle_out_of_memory_signals(out_of_memory_triggers)
                .await;
        }
        if !process_end_triggers.is_empty() {
            debug!(
                "Processing {} finishing processes",
                process_end_triggers.len()
            );

            self.process_manager
                .write()
                .await
                .handle_out_of_memory_terminations(&mut process_end_triggers)
                .await;

            self.process_manager
                .write()
                .await
                .handle_process_terminations(process_end_triggers)
                .await?;
        }

        // Then process start triggers
        if !process_start_triggers.is_empty() {
            debug!(
                "Processing {} creating processes",
                process_start_triggers.len()
            );
            self.process_manager
                .write()
                .await
                .handle_process_starts(process_start_triggers)
                .await?;
        }

        Ok(())
    }

    pub async fn poll_process_metrics(&self) -> Result<()> {
        self.process_manager
            .write()
            .await
            .poll_process_metrics()
            .await
    }

    pub async fn preview_targets(&self, n: usize) -> HashSet<String> {
        self.process_manager.write().await.preview_targets(n).await
    }

    pub async fn targets_len(&self) -> usize {
        self.process_manager.write().await.targets_len().await
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

    // Helper function to create a mock FileWatcher
    fn create_mock_file_watcher() -> Arc<RwLock<FileWatcher>> {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        Arc::new(RwLock::new(FileWatcher::new(temp_dir)))
    }

    // Helper function to set up a process watcher with specified targets and processes
    fn setup_process_watcher(
        target_manager: TargetManager,
        processes: HashMap<usize, ProcessStartTrigger>,
    ) -> Arc<ProcessWatcher> {
        let state = ProcessState {
            processes,
            monitoring: HashMap::new(),
            target_manager,
            oom_victims: HashMap::new(),
            ebpf_task: None,
        };

        let log_recorder = create_mock_log_recorder();
        let file_watcher = create_mock_file_watcher();
        let state = Arc::new(RwLock::new(state));

        Arc::new(ProcessWatcher {
            ebpf: once_cell::sync::OnceCell::new(),
            log_recorder,
            file_watcher,
            system,
            state,
            process_manager: Non,
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
        #[case] process: ProcessStartTrigger,
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
    async fn test_nextflow_wrapped_scripts(#[case] process: ProcessStartTrigger) {
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
    fn dummy_process(name: &str, cmd: &str, path: &str) -> ProcessStartTrigger {
        ProcessStartTrigger {
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
