use crate::common::recorder::LogRecorder;
use crate::common::target_process::Target;
use crate::common::types::event::attributes::EventAttributes;
use crate::common::types::event::ProcessStatus as TracerProcessStatus;
use crate::extracts::process::extract_process_data::ExtractProcessData;
use crate::extracts::process::process_utils::create_short_lived_process_properties;
use crate::extracts::process::types::process_result::ProcessResult;
use anyhow::Result;
use chrono::Utc;
use sysinfo::Process;
use tracer_ebpf::ebpf_trigger::{ProcessEndTrigger, ProcessStartTrigger};
use tracing::debug;

/// Handles logging of process-related events
pub struct ProcessLogger {
    log_recorder: LogRecorder,
}

impl ProcessLogger {
    pub fn new(log_recorder: LogRecorder) -> Self {
        Self { log_recorder }
    }

    /// Logs information about a newly detected process
    pub async fn log_new_process(
        &self,
        target: &Target,
        process: &ProcessStartTrigger,
        system_process: Option<&Process>,
    ) -> Result<ProcessResult> {
        debug!("Processing pid={}", process.pid);

        let display_name = target
            .get_display_name_object()
            .get_display_name(&process.file_name, process.argv.as_slice());

        let properties = match system_process {
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

    /// Logs metrics update for an already running process
    pub async fn log_process_metrics(
        &self,
        target: &Target,
        process: &ProcessStartTrigger,
        system_process: Option<&Process>,
    ) -> Result<ProcessResult> {
        let display_name = target
            .get_display_name_object()
            .get_display_name(&process.file_name, process.argv.as_slice());

        let Some(system_process) = system_process else {
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
        let properties = ExtractProcessData::gather_process_data(
            system_process,
            display_name.clone(),
            process.started_at,
        )
        .await;

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

    /// Logs completion of a process
    pub async fn log_process_completion(
        &self,
        start_trigger: &ProcessStartTrigger,
        finish_trigger: &ProcessEndTrigger,
    ) -> Result<()> {
        let duration_sec = (finish_trigger.finished_at - start_trigger.started_at)
            .num_seconds()
            .try_into()
            .unwrap_or(0);

        let properties = crate::common::types::event::attributes::process::CompletedProcess {
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
