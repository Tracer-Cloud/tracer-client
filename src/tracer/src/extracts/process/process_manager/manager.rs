use crate::common::recorder::LogRecorder;
use crate::common::target_process::target_process_manager::TargetManager;
use crate::common::target_process::Target;
use crate::extracts::process::process_manager::handlers::oom::OomHandler;
use crate::extracts::process::process_manager::handlers::process_starts::ProcessStartHandler;
use crate::extracts::process::process_manager::handlers::process_terminations::ProcessTerminationHandler;
use crate::extracts::process::process_manager::logger::ProcessLogger;
use crate::extracts::process::process_manager::matcher::Filter;
use crate::extracts::process::process_manager::metrics::ProcessMetricsHandler;
use crate::extracts::process::process_manager::state::StateManager;
use crate::extracts::process::process_manager::system_refresher::SystemRefresher;
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

    /// Finds matching processes (functional approach with explicit data flow)
    pub async fn find_matching_processes(
        &self,
        triggers: Vec<ProcessStartTrigger>,
    ) -> Result<HashMap<Target, HashSet<ProcessStartTrigger>>> {
        let state = self.state_manager.get_state().await;
        self.matcher.find_matching_processes(triggers, &state)
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
    use tokio::sync::{mpsc, RwLock};

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

        // Test the function using functional approach
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
        // Create a target and set up the process manager
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

        // Test the function using functional approach
        let result = process_manager
            .find_matching_processes(vec![process])
            .await
            .unwrap();

        // Assert no processes were matched
        assert_eq!(result.len(), 0);
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

        // Set up the process manager
        let target_manager = TargetManager::new(vec![target], vec![]);
        let log_recorder = create_mock_log_recorder();
        let process_manager = ProcessManager::new(target_manager, log_recorder);

        // Insert the parent process into the state first
        process_manager
            .state_manager
            .insert_process(parent_process.pid, parent_process)
            .await;

        // Test with the child process using functional approach
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
