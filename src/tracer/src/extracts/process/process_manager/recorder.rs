use crate::extracts::containers::DockerWatcher;
use crate::extracts::process::extract_process_data;
use crate::extracts::process::extract_process_data::construct_tool_id;
use crate::extracts::process::types::process_result::ProcessResult;
use crate::process_identification::recorder::EventDispatcher;
use crate::process_identification::target_pipeline::pipeline_manager::TaskMatch;
use crate::process_identification::types::event::attributes::process::ProcessProperties;
use crate::process_identification::types::event::attributes::EventAttributes;
use crate::process_identification::types::event::ProcessStatus as TracerProcessStatus;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use sysinfo::Process;
use tracer_ebpf::ebpf_trigger::{ProcessEndTrigger, ProcessStartTrigger};
use tracing::debug;
use tracing::error;

/// Handles recording of process-related events
pub struct EventRecorder {
    event_dispatcher: EventDispatcher,
    /// shared reference to the docker watcher - used to get the ContainerEvent associated
    /// with a process
    docker_watcher: Arc<DockerWatcher>,
}

impl EventRecorder {
    pub fn new(event_dispatcher: EventDispatcher, docker_watcher: Arc<DockerWatcher>) -> Self {
        Self {
            event_dispatcher,
            docker_watcher,
        }
    }

    pub fn get_trace_id(&self) -> &Option<String> {
        self.event_dispatcher.get_trace_id()
    }

    /// Records information about a newly detected process
    pub async fn record_new_process(
        &self,
        target: &String,
        process: &ProcessStartTrigger,
        system_process: Option<&Process>,
    ) -> Result<ProcessResult> {
        debug!("Processing pid={}", process.pid);

        let display_name = target;

        let mut properties = match system_process {
            Some(system_process) => {
                extract_process_data::gather_process_data(
                    system_process,
                    display_name.clone(),
                    process.started_at,
                    &process.argv,
                    &process.env,
                )
                .await
            }
            None => {
                debug!("Process({}) wasn't found", process.pid);
                extract_process_data::create_short_lived_process_object(
                    process,
                    display_name.clone(),
                )
            }
        };

        let ProcessProperties::Full(full) = &mut properties;

        // If we have a container ID, fetch and attach the container event
        if let Some(container_id) = &full.container_id {
            if let Some(container_event) =
                self.docker_watcher.get_container_event(container_id).await
            {
                full.container_event = Some(container_event);
            }
        }

        self.event_dispatcher
            .log(
                TracerProcessStatus::ToolExecution,
                format!("[{}] Tool process: {}", Utc::now(), &display_name),
                Some(EventAttributes::Process(properties)),
                None,
            )
            .await?;

        Ok(ProcessResult::Found)
    }

    /// Records metrics update for an already running process
    pub async fn record_process_metrics(
        &self,
        target: &String,
        process: &ProcessStartTrigger,
        system_process: Option<&Process>,
    ) -> Result<ProcessResult> {
        let display_name = target;
        let Some(system_process) = system_process else {
            // Process no longer exists
            debug!("Process wasn't found {}", target);
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
        let properties = extract_process_data::gather_process_data(
            system_process,
            display_name.clone(),
            process.started_at,
            &process.argv,
            &process.env,
        )
        .await;

        debug!("Process data completed. PID={}", process.pid);

        self.event_dispatcher
            .log(
                TracerProcessStatus::ToolMetricEvent,
                format!("[{}] Tool metric event: {}", Utc::now(), &display_name),
                Some(EventAttributes::Process(properties)),
                None,
            )
            .await?;

        Ok(ProcessResult::Found)
    }

    /// Records completion of a process
    pub async fn record_process_completion(
        &self,
        target: &str,
        start_trigger: &ProcessStartTrigger,
        finish_trigger: &ProcessEndTrigger,
    ) -> Result<()> {
        let duration_sec = (finish_trigger.finished_at - start_trigger.started_at)
            .num_seconds()
            .try_into()
            .unwrap_or(0);

        error!(
            "record_process_completion: START: finish trigger: {:?}",
            finish_trigger
        );

        // CompletedProcess contains the exit reason, the tool_id, the tool_name, and started and ended at
        // started and ended at might not seem very useful, but might help in the future with duration calculations
        let properties =
            crate::process_identification::types::event::attributes::process::CompletedProcess {
                tool_id: construct_tool_id(
                    &start_trigger.pid.to_string(),
                    start_trigger.started_at,
                ),
                tool_name: target.to_owned(),
                tool_pid: start_trigger.pid.to_string(),
                duration_sec,
                exit_reason: finish_trigger.exit_reason.clone(),
                started_at: start_trigger.started_at,
                ended_at: finish_trigger.finished_at,
            };

        self.event_dispatcher
            .log(
                TracerProcessStatus::FinishedToolExecution,
                format!("[{}] {} exited", Utc::now(), &start_trigger.comm),
                Some(EventAttributes::CompletedProcess(properties)),
                None,
            )
            .await?;

        Ok(())
    }

    /// Record a match for a set of processes to a job.
    pub async fn record_task_match(&self, task_match: TaskMatch) -> Result<()> {
        self.event_dispatcher
            .log(
                TracerProcessStatus::TaskMatch,
                format!("[{}] Job match: {}", Utc::now(), &task_match),
                Some(EventAttributes::TaskMatch(task_match)),
                None,
            )
            .await
    }
}
