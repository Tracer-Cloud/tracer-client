use tracer_common::event::ProcessStatus as TracerProcessStatus;

use crate::data_samples::DATA_SAMPLES_EXT;
use crate::file_watcher::FileWatcher;
use anyhow::Result;
use chrono::{DateTime, Utc};
use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::{hash_map::Entry::Vacant, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessStatus, System};
use tokio::sync::{mpsc, RwLock};
use tracer_common::event::attributes::process::{
    CompletedProcess, DataSetsProcessed, FullProcessProperties, InputFile, ProcessProperties,
    ShortProcessProperties,
};
use tracer_common::event::attributes::EventAttributes;
use tracer_common::recorder::StructLogRecorder;
use tracer_common::target_process::{Target, TargetMatchable};
use tracer_common::trigger::{ProcessTrigger, Trigger};
use tracer_ebpf_user::{start_processing_events, TracerEbpf};
use tracing::{debug, error, info};

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

struct ProcessState {
    processes: HashMap<usize, ProcessTrigger>, // todo: use (pid, starttime)
    monitoring: Vec<(Target, HashSet<ProcessTrigger>)>, // todo: avoid target copy

    targets: Vec<Target>,
}

impl ProcessState {
    fn current(&self) -> HashSet<usize> {
        self.monitoring
            .iter()
            .flat_map(|(_, processes)| processes.iter().map(|p| p.pid))
            .collect()
    }
}

pub struct ProcessWatcher {
    ebpf: once_cell::sync::OnceCell<TracerEbpf>, // not tokio, because TracerEbpf is sync
    log_recorder: StructLogRecorder,
    file_watcher: Arc<RwLock<FileWatcher>>,
    system: Arc<RwLock<System>>,

    state: Arc<RwLock<ProcessState>>,
}

impl ProcessWatcher {
    pub fn new(
        targets: Vec<Target>,
        log_recorder: StructLogRecorder,
        file_watcher: Arc<RwLock<FileWatcher>>,
        system: Arc<RwLock<System>>,
    ) -> Self {
        let state = Arc::new(RwLock::new(ProcessState {
            processes: HashMap::new(),
            monitoring: Vec::new(),
            targets: targets.clone(),
        }));

        ProcessWatcher {
            ebpf: once_cell::sync::OnceCell::new(),
            log_recorder,
            file_watcher,
            system,
            state,
        }
    }

    pub async fn update_targets(self: &Arc<Self>, targets: Vec<Target>) -> Result<()> {
        let mut state = self.state.write().await;

        state.targets = targets;
        // todo: before, we'd clean seen. should we do it now?

        Ok(())
    }

    pub async fn start_ebpf(self: &Arc<Self>) -> Result<()> {
        Arc::clone(&self)
            .ebpf
            .get_or_try_init(|| Arc::clone(self).initialize_ebpf())?;
        Ok(())
    }

    fn initialize_ebpf(self: Arc<Self>) -> Result<TracerEbpf, anyhow::Error> {
        let (tx, mut rx) = mpsc::channel::<Trigger>(100);
        let ebpf = start_processing_events(tx.clone())?;

        let watcher = Arc::clone(&self);
        tokio::spawn(async move {
            watcher.process_trigger_loop(rx).await;
        });

        Ok(ebpf)
    }

    async fn process_trigger_loop(self: &Arc<Self>, mut rx: mpsc::Receiver<Trigger>) {
        let mut buff: Vec<Trigger> = Vec::with_capacity(100);

        loop {
            buff.clear();

            while rx.recv_many(&mut buff, 100).await > 0 {
                let s = std::mem::take(&mut buff);
                println!("^Received {:?}", s);
                println!("$s {:?}", self.state.read().await.processes);
                if let Err(e) = self.process_triggers(s).await {
                    println!("Failed to process triggers: {}", e);
                }
            }
        }
    }

    async fn process_triggers(self: &Arc<ProcessWatcher>, buff: Vec<Trigger>) -> Result<()> {
        let mut processes: Vec<ProcessTrigger> = vec![];

        for trigger in buff.into_iter() {
            match trigger {
                Trigger::Start(proc) => {
                    processes.push(proc);
                }
            }
        }

        println!("processes after filter: {}", processes.len());

        self.process_start(processes).await?;

        Ok(())
    }

    async fn process_start(self: &Arc<ProcessWatcher>, buff: Vec<ProcessTrigger>) -> Result<()> {
        debug!("processing {} processes", buff.len());

        let interested_in = self.process_start_processes(buff).await?;

        println!(
            "after refreshing, interested in {} processes",
            interested_in.len()
        );

        if interested_in.is_empty() {
            return Ok(());
        }

        self.refresh_system(&interested_in).await?;

        for (target, triggers) in interested_in.iter() {
            for process in triggers.iter() {
                self.process_new_process(target, process).await?;
            }
        }

        let mut state = self.state.write().await;
        // state.monitoring = interested_in; // TODO: FIXME, INSTEAD OF REPLACING, WE SHOULD MERGE IT
        state.monitoring.extend(interested_in);

        Ok(())
    }

    async fn process_start_processes(
        self: &Arc<ProcessWatcher>,
        buff: Vec<ProcessTrigger>,
    ) -> Result<Vec<(Target, HashSet<ProcessTrigger>)>> {
        {
            let mut state = self.state.write().await;

            for trigger in buff.iter() {
                state.processes.insert(trigger.pid, trigger.clone());
            }
        }

        let state = self.state.read().await;
        let already_seen: HashSet<usize> = state
            .monitoring
            .iter()
            .flat_map(|(_, processes)| processes.iter().map(|p| p.pid))
            .collect();

        let matched_processes = self.match_new_processes(buff).await?;
        println!("matched_processes={:?}", matched_processes);

        let interested_in: Vec<(_, _)> = matched_processes
            .into_iter()
            // add parents, remove those are already in self.monitoring
            .map(|(target, processes)| {
                let processes = processes
                    .into_iter()
                    .flat_map(|proc| {
                        let mut parents = Self::get_with_parents(&state, proc);
                        parents.retain(|p| !already_seen.contains(&p.pid));
                        parents
                    })
                    .collect::<HashSet<_>>();

                (target, processes)
            })
            .collect();

        Ok(interested_in)
    }

    async fn refresh_system(
        self: &Arc<ProcessWatcher>,
        targets: &Vec<(Target, HashSet<ProcessTrigger>)>,
    ) -> Result<()> {
        println!("refreshing {} processes", targets.len());

        let to_enrich: HashSet<usize> = targets
            .iter()
            .flat_map(|(_, processes)| processes.iter().map(|p| p.pid))
            .collect();

        let pids = to_enrich
            .iter()
            .map(|pid| Pid::from(*pid))
            .collect::<Vec<_>>();

        let mut system = self.system.write().await;

        system.refresh_pids_specifics(
            // todo: tokio::task::spawn_blocking(
            pids.as_slice(),
            ProcessRefreshKind::everything(), // todo: minify
        );

        Ok(())
    }

    fn get_with_parents(state: &ProcessState, proc: ProcessTrigger) -> HashSet<ProcessTrigger> {
        let mut current_pid = proc.ppid;
        let mut parents = HashSet::new();
        parents.insert(proc);

        while let Some(parent) = state.processes.get(&current_pid) {
            current_pid = parent.ppid;
            parents.insert(parent.clone());
        }
        parents
    }

    async fn match_new_processes(
        self: &Arc<ProcessWatcher>,
        triggers: Vec<ProcessTrigger>,
    ) -> Result<Vec<(Target, HashSet<ProcessTrigger>)>> {
        let mut matched_processes = vec![];
        let targets = &self.state.read().await.targets;

        for target in targets {
            let mut matches: HashSet<ProcessTrigger> = HashSet::new();

            for trigger in triggers.iter() {
                if true
                    || target.matches(
                        // TODO
                        &trigger.comm,
                        &trigger.file_name,
                        &trigger.argv.join(" "),
                    )
                {
                    matches.insert(trigger.clone());
                }
            }

            if !matches.is_empty() {
                matched_processes.push((target.clone(), matches));
            }

            break; // todo
        }

        Ok(matched_processes)
    }

    async fn process_new_process(
        self: &Arc<ProcessWatcher>,
        target: &Target,
        process: &ProcessTrigger,
    ) -> Result<ProcessResult> {
        println!("processing {} processes", process.pid);

        let name = target
            .get_display_name_object()
            .get_display_name(&process.file_name, process.argv.as_slice()); // todo :fixme

        let properties = {
            let system = self.system.read().await;

            match system.process(process.pid.into()) {
                Some(system_process) => {
                    self.gather_process_data(&system_process, name.clone(), true)
                        .await
                }
                None => {
                    debug!("Process({}) wasn't found", process.pid);

                    ProcessProperties::ShortLived(ShortProcessProperties {
                        tool_name: name.clone(),
                        tool_pid: process.pid.to_string(), // todo: use ints
                        tool_parent_pid: process.ppid.to_string(),
                        tool_binary_path: process.file_name.clone(),
                        start_timestamp: Utc::now().to_rfc3339(),
                    })
                }
            }
        };

        println!("log properties, pid={}", process.pid);

        self.log_recorder
            .log(
                TracerProcessStatus::ToolExecution,
                format!("[{}] Tool process: {}", Utc::now(), &name),
                Some(EventAttributes::Process(properties)),
                None,
            )
            .await?;

        // self.log_datasets_in_process().await?; // todo:
        Ok(ProcessResult::Found)
    }

    async fn process_running_process(
        self: &Arc<ProcessWatcher>,
        target: &Target,
        process: &ProcessTrigger,
    ) -> Result<ProcessResult> {
        let name = target
            .get_display_name_object()
            .get_display_name(&process.file_name, process.argv.as_slice()); // todo :fixme

        let properties = {
            let system = self.system.read().await;

            let Some(system_process) = system.process(process.pid.into()) else {
                error!("Process({}) wasn't found", process.pid);
                // todo: fill short_lived_process?
                return Ok(ProcessResult::NotFound);
            };

            self.gather_process_data(&system_process, name.clone(), false)
                .await
        };

        self.log_recorder
            .log(
                TracerProcessStatus::ToolMetricEvent,
                format!("[{}] Tool metric event: {}", Utc::now(), &name),
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
        let start_time = Utc::now(); // todo: use proc starttime

        let (container_id, job_id, trace_id) = Self::read_process_env(proc);
        let working_directory = proc.cwd().map(|p| p.to_string_lossy().to_string());

        let input_files = if process_input_files {
            let mut files = vec![];
            let cmd_arguments = proc.cmd();

            let mut arguments_to_check = vec![];

            for arg in cmd_arguments {
                if arg.starts_with('-') {
                    continue;
                }

                if arg.contains('=') {
                    let split: Vec<&str> = arg.split('=').collect();
                    if split.len() > 1 {
                        arguments_to_check.push(split[1]);
                    }
                }
                arguments_to_check.push(arg);
            }

            let watcher = self.file_watcher.read().await;
            for arg in arguments_to_check {
                let file = watcher.get_file_by_path_suffix(arg);
                if let Some((path, file_info)) = file {
                    files.push(InputFile {
                        file_name: file_info.name.clone(),
                        file_size: file_info.size,
                        file_path: path.clone(),
                        file_directory: file_info.directory.clone(),
                        file_updated_at_timestamp: file_info.last_update.to_rfc3339(),
                    });
                }
            }

            Some(files)
        } else {
            None
        };

        ProcessProperties::Full(FullProcessProperties {
            tool_name: display_name,
            tool_pid: proc.pid().as_u32().to_string(),
            tool_parent_pid: proc.parent().unwrap_or(0.into()).to_string(),
            tool_binary_path: proc
                .exe()
                .unwrap_or_else(|| Path::new(""))
                .as_os_str()
                .to_str()
                .unwrap()
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
            input_files: input_files,
            container_id,
            job_id,
            working_directory,
            trace_id,
        })
    }

    fn read_process_env(proc: &Process) -> (Option<String>, Option<String>, Option<String>) {
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

    pub async fn refresh_process(self: &Arc<ProcessWatcher>, pid: &Pid) -> Result<()> {
        let mut system = self.system.read().await;

        Ok(())
    }

    async fn on_process_terminated(self: &Arc<ProcessWatcher>, pid: &Pid) -> Result<()> {
        todo!()
    }

    async fn log_datasets_in_process(self: &Arc<Self>) -> Result<()> {
        todo!()
    }

    async fn poll_process_metrics(self: &Arc<Self>) -> Result<()> {
        let state = self.state.read().await;
        self.refresh_system(&state.monitoring).await?;

        Ok(())
    }

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

    pub async fn targets_len(&self) -> usize {
        self.state
            .read()
            .await
            .monitoring
            .iter()
            .map(|(_, processes)| processes.len())
            .sum()
    }
}
