use tracer_common::types::event::ProcessStatus as TracerProcessStatus;

use crate::data_samples::DATA_SAMPLES_EXT;
use crate::file_watcher::FileWatcher;
use anyhow::Result;
use chrono::{DateTime, Utc};
use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::{hash_map::Entry::Vacant, HashSet};
use std::ops::Sub;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessStatus, System};
use tokio::sync::{mpsc, RwLock};
use tracer_common::recorder::StructLogRecorder;
use tracer_common::target_process::{Target, TargetMatchable};
use tracer_common::trigger::{FinishTrigger, ProcessTrigger, Trigger};
use tracer_common::types::event::attributes::process::{
    CompletedProcess, DataSetsProcessed, FullProcessProperties, InputFile, ProcessProperties,
    ShortProcessProperties,
};
use tracer_common::types::event::attributes::EventAttributes;
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
    monitoring: HashMap<Target, HashSet<ProcessTrigger>>, // todo: avoid target copy
    datasamples_tracker: HashMap<String, HashSet<String>>, // this hashmap groups datasets by the nextflow session uuid

    targets: Vec<Target>,
}

impl ProcessState {
    fn current(&self) -> HashSet<usize> {
        self.monitoring
            .values()
            .flat_map(|processes| processes.iter().map(|p| p.pid))
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
            debug!("Ready to receive triggers");

            while rx.recv_many(&mut buff, 100).await > 0 {
                let s = std::mem::take(&mut buff);
                debug!("Received {:?}", s);
                if let Err(e) = self.process_triggers(s).await {
                    error!("Failed to process triggers: {}", e);
                }
            }
        }
    }

    async fn process_triggers(self: &Arc<ProcessWatcher>, buff: Vec<Trigger>) -> Result<()> {
        let mut start: Vec<ProcessTrigger> = vec![];
        let mut finish: Vec<FinishTrigger> = vec![];

        for trigger in buff.into_iter() {
            match trigger {
                Trigger::Start(proc) => {
                    start.push(proc);
                }
                Trigger::Finish(proc) => {
                    finish.push(proc);
                }
            }
        }

        if !finish.is_empty() {
            debug!("processing {} finishing processes", finish.len());
            self.process_termination(finish).await?;
        }

        if !start.is_empty() {
            debug!("processing {} creating processes", start.len());
            self.process_start(start).await?;
        }

        Ok(())
    }

    async fn remove_processes(self: &Arc<Self>, buff: &Vec<FinishTrigger>) -> Result<()> {
        let mut state = self.state.write().await;
        for trigger in buff.iter() {
            state.processes.remove(&trigger.pid);
        }

        Ok(())
    }

    async fn process_termination(self: &Arc<Self>, buff: Vec<FinishTrigger>) -> Result<()> {
        debug!("processing {} creating processes", buff.len());
        self.remove_processes(&buff).await?;

        let mut buff: HashMap<_, _> = buff.into_iter().map(|proc| (proc.pid, proc)).collect();

        let taken: HashSet<_> = {
            let mut state = self.state.write().await;

            state
                .monitoring
                .iter_mut()
                .flat_map(|(_, procs)| {
                    let (removed, retained): (Vec<_>, Vec<_>) = procs
                        .drain()
                        .partition(|proc| buff.keys().contains(&proc.pid));
                    *procs = retained.into_iter().collect();
                    removed
                })
                .collect()
        };

        debug!("removed {} processes. taken={:?}, buff={:?}", taken.len(), taken, buff);

        for start in taken {
            let finish: FinishTrigger = buff
                .remove(&start.pid)
                .expect("Process should be present in the map");

            // should be safe since
            // - we've checked the key is present
            // - we have an exclusive lock on the state
            // - if trigger is duplicated in monitoring (can happened if it matches several targets),
            //   it'll be deduplicated via hashset

            self.process_proc_finish(&start, &finish).await?;
        }

        Ok(())
    }

    async fn process_proc_finish(
        self: &Arc<Self>,
        start: &ProcessTrigger,
        end: &FinishTrigger,
    ) -> Result<()> {
        let duration_sec = (end.finished_at - start.started_at)
            .num_seconds()
            .try_into()
            .unwrap_or(0);

        let properties = CompletedProcess {
            tool_name: start.comm.clone(), // todo: use tool name instead
            tool_pid: start.pid.to_string(),
            duration_sec,
        };

        self.log_recorder
            .log(
                TracerProcessStatus::FinishedToolExecution,
                format!("[{}] {} exited", Utc::now(), &start.comm),
                Some(EventAttributes::CompletedProcess(properties)),
                None,
            )
            .await?;

        Ok(())
    }

    async fn process_start(self: &Arc<ProcessWatcher>, buff: Vec<ProcessTrigger>) -> Result<()> {
        debug!("processing {} creating processes", buff.len());

        let interested_in = self.process_start_processes(buff).await?;

        debug!(
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

        // merge old and new targets
        for (k, v) in interested_in.into_iter() {
            state.monitoring.entry(k).or_default().extend(v);
        }

        Ok(())
    }

    async fn process_start_processes(
        self: &Arc<ProcessWatcher>,
        buff: Vec<ProcessTrigger>,
    ) -> Result<HashMap<Target, HashSet<ProcessTrigger>>> {
        {
            let mut state = self.state.write().await;

            for trigger in buff.iter() {
                state.processes.insert(trigger.pid, trigger.clone());
            }
        }

        let state = self.state.read().await;
        let already_seen: HashSet<usize> = state
            .monitoring
            .values()
            .flat_map(|processes| processes.iter().map(|p| p.pid))
            .collect();

        let matched_processes = self.match_new_processes(buff).await?;

        let interested_in: HashMap<_, _> = matched_processes
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
        targets: &HashMap<Target, HashSet<ProcessTrigger>>,
    ) -> Result<()> {
        debug!("refreshing {} processes", targets.len());

        let to_enrich: HashSet<usize> = targets
            .values()
            .flat_map(|processes| processes.iter().map(|p| p.pid))
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
                if target.matches(&trigger.comm, &trigger.argv.join(" "), &trigger.file_name) {
                    matches.insert(trigger.clone());
                }
            }

            if !matches.is_empty() {
                matched_processes.push((target.clone(), matches));
            }
        }

        Ok(matched_processes)
    }

    async fn process_new_process(
        self: &Arc<ProcessWatcher>,
        target: &Target,
        process: &ProcessTrigger,
    ) -> Result<ProcessResult> {
        debug!("processing pid={}", process.pid);

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

        if let ProcessProperties::Full(ref properties) = properties {
            self.log_datasets_in_process(&process.argv, properties)
                .await?;
        }

        self.log_recorder
            .log(
                TracerProcessStatus::ToolExecution,
                format!("[{}] Tool process: {}", Utc::now(), &name),
                Some(EventAttributes::Process(properties)),
                None,
            )
            .await?;

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
                // info!(
                //     "Process {} wasn't found when updating: assuming it finished",
                //     process.pid
                // );
                return Ok(ProcessResult::NotFound);
            };

            debug!(
                "Loaded process. PID: ebpf={}, system={:?}; Start Time:  ebpf={}, system={:?};",
                process.pid,
                system_process.pid(),
                process.started_at.timestamp(),
                system_process.start_time()
            );

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

    async fn log_datasets_in_process(
        self: &Arc<Self>,
        cmd: &[String],
        properties: &FullProcessProperties,
    ) -> Result<()> {
        let trace_id: Option<String> = properties.trace_id.clone();
        let datasamples_tracker = &mut self.state.write().await.datasamples_tracker;

        for arg in cmd.iter() {
            if DATA_SAMPLES_EXT.iter().any(|ext| arg.ends_with(ext)) {
                datasamples_tracker
                    .entry(trace_id.clone().unwrap_or_default())
                    .or_default()
                    .insert(arg.clone());
            }
        }

        // TODO change this logic
        let properties = DataSetsProcessed {
            datasets: datasamples_tracker
                .get(&trace_id.clone().unwrap_or_default())
                .map(|set| set.iter().cloned().collect::<Vec<_>>().join(", "))
                .unwrap_or_default(),
            total: datasamples_tracker
                .get(&trace_id.clone().unwrap_or_default())
                .unwrap_or(&HashSet::new())
                .len() as u64,
            trace_id,
        };

        self.log_recorder
            .log(
                TracerProcessStatus::DataSamplesEvent,
                format!("[{}] Samples Processed So Far", Utc::now()),
                Some(EventAttributes::ProcessDatasetStats(properties)),
                None,
            )
            .await
    }

    pub async fn poll_process_metrics(self: &Arc<Self>) -> Result<()> {
        let state = self.state.read().await;

        if state.monitoring.is_empty() {
            debug!("No processes to monitor, skipping poll");
            return Ok(());
        }

        debug!("Refreshing data for {} processes", state.monitoring.len());
        self.refresh_system(&state.monitoring).await?;
        self.process_updates().await?;

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

    async fn process_updates(self: &Arc<ProcessWatcher>) -> Result<()> {
        for (target, procs) in self.state.read().await.monitoring.iter() {
            for proc in procs.iter() {
                let result = self.process_running_process(target, proc).await?;

                match result {
                    ProcessResult::NotFound => {
                        // todo: mark process as completed
                        debug!("Process {} was not found", proc.pid);
                    }
                    ProcessResult::Found => {}
                }
            }
        }

        Ok(())
    }
}
