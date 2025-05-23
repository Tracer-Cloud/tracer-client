use anyhow::Result;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use sysinfo::{Process, ProcessStatus, System};
use tokio::sync::RwLock;
use tracing::{debug};
use tracer_common::recorder::LogRecorder;
use tracer_common::target_process::{Target, TargetMatchable};
use tracer_common::types::event::attributes::process::{
    CompletedProcess, FullProcessProperties, InputFile, ProcessProperties, ShortProcessProperties,
};
use tracer_common::types::event::attributes::EventAttributes;
use tracer_common::types::event::ProcessStatus as TracerProcessStatus;
use tracer_common::types::trigger::{FinishTrigger, ProcessTrigger};
use crate::metrics::extract_variables::Extract;

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

/// Internal state of the process manager
struct ProcessState {
    // Maps PIDs to process triggers
    processes: HashMap<usize, ProcessTrigger>,
    // Maps targets to sets of processes being monitored
    monitoring: HashMap<Target, HashSet<ProcessTrigger>>,
}

/// Manages process lifecycle and metrics
pub struct ProcessManager {
    log_recorder: LogRecorder,
    system: Arc<RwLock<System>>,
    state: Arc<RwLock<ProcessState>>,
}

impl ProcessManager {
    pub fn new(
        log_recorder: LogRecorder,
        system: Arc<RwLock<System>>,
    ) -> Self {
        let state = Arc::new(RwLock::new(ProcessState {
            processes: HashMap::new(),
            monitoring: HashMap::new(),
        }));

        ProcessManager {
            log_recorder,
            system,
            state,
        }
    }

    /// Handles process start events
    pub async fn handle_process_start(
        self: &Arc<Self>,
        target: &Target,
        process: &ProcessTrigger,
    ) -> Result<()> {
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

        // Add to monitoring
        let mut state = self.state.write().await;
        state
            .monitoring
            .entry(target.clone())
            .or_default()
            .insert(process.clone());

        Ok(())
    }

    /// Handles process end events
    pub async fn handle_process_end(
        self: &Arc<Self>,
        finish_trigger: &FinishTrigger,
    ) -> Result<()> {
        let mut state = self.state.write().await;
        
        // Find the process in monitoring and remove it
        let mut process_to_remove = None;
        for (target, processes) in state.monitoring.iter_mut() {
            // First, check if the process exists and clone the data we need
            if let Some(process) = processes.iter().find(|p| p.pid == finish_trigger.pid) {
                process_to_remove = Some((target.clone(), process.clone()));
            }
            
            // Then remove the process (this is separate from the find operation)
            if process_to_remove.is_some() {
                processes.retain(|p| p.pid != finish_trigger.pid);
                break;
            }
        }

        if let Some((target, process)) = process_to_remove {
            let duration_sec = (finish_trigger.finished_at - process.started_at)
                .num_seconds()
                .try_into()
                .unwrap_or(0);

            let properties = CompletedProcess {
                tool_name: process.comm.clone(),
                tool_pid: process.pid.to_string(),
                duration_sec,
            };

            self.log_recorder
                .log(
                    TracerProcessStatus::FinishedToolExecution,
                    format!("[{}] {} exited", Utc::now(), &process.comm),
                    Some(EventAttributes::CompletedProcess(properties)),
                    None,
                )
                .await?;
        }

        // Remove from processes map
        state.processes.remove(&finish_trigger.pid);

        Ok(())
    }

    /// Updates metrics for all monitored processes
    pub async fn update_all_processes(self: &Arc<Self>) -> Result<()> {
        for (target, procs) in self.state.read().await.monitoring.iter() {
            for proc in procs.iter() {
                if let Err(e) = self.update_running_process(target, proc).await {
                    debug!("Failed to update process {}: {}", proc.pid, e);
                }
            }
        }

        Ok(())
    }

    /// Updates metrics for a single running process
    async fn update_running_process(
        self: &Arc<Self>,
        target: &Target,
        process: &ProcessTrigger,
    ) -> Result<()> {
        let display_name = target
            .get_display_name_object()
            .get_display_name(&process.file_name, process.argv.as_slice());

        let properties = {
            let system = self.system.read().await;

            let Some(system_process) = system.process(process.pid.into()) else {
                debug!("Process {} was not found during update", process.pid);
                return Ok(()); // Process no longer exists, that's okay
            };

            self.gather_process_data(
                system_process,
                display_name.clone(),
                false,
                process.started_at,
            )
            .await
        };

        self.log_recorder
            .log(
                TracerProcessStatus::ToolMetricEvent,
                format!("[{}] Tool metric event: {}", Utc::now(), &display_name),
                Some(EventAttributes::Process(properties)),
                None,
            )
            .await?;

        Ok(())
    }

    /// Gathers process data and creates process properties
    async fn gather_process_data(
        &self,
        proc: &Process,
        display_name: String,
        process_input_files: bool,
        process_start_time: chrono::DateTime<Utc>,
    ) -> ProcessProperties {
        let (container_id, job_id, trace_id) = Extract::extract_process_env_vars(proc);

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
            process_run_time,
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

    /// Creates properties for a short-lived process
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

    /// Extracts input files from a process
    async fn extract_input_files(&self, proc: &Process) -> Option<Vec<InputFile>> {
        // TODO: Implement input file extraction
        None
    }

    /// Logs datasets found in process
    async fn log_datasets_in_process(
        &self,
        argv: &[String],
        properties: &FullProcessProperties,
    ) -> Result<()> {
        // TODO: Implement dataset logging
        Ok(())
    }

    /// Returns the number of processes being monitored
    pub async fn monitored_processes_count(&self) -> usize {
        self.state
            .read()
            .await
            .monitoring
            .values()
            .map(|processes| processes.len())
            .sum()
    }
}
