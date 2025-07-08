use crate::extracts::containers::DockerWatcher;
use crate::extracts::ebpf_watcher::handler::trigger::trigger_processor::TriggerProcessor;
use crate::extracts::process::process_manager::ProcessManager;
use crate::extracts::process::process_utils::get_process_argv;
use crate::process_identification::recorder::LogRecorder;
use anyhow::{Error, Result};
use std::collections::HashSet;
use std::fs::{self};
use std::path::Path;
use std::sync::Arc;
use sysinfo::ProcessesToUpdate;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracer_ebpf::binding::start_processing_events;
use tracer_ebpf::ebpf_trigger::{
    OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger, Trigger,
};
use tracing::{debug, error, info};

/// Watches system processes and records events related to them
pub struct EbpfWatcher {
    ebpf_initialized: Arc<Mutex<bool>>,
    process_manager: Arc<RwLock<ProcessManager>>,
    trigger_processor: TriggerProcessor,
    // here will go the file manager for dataset recognition operations
}

impl EbpfWatcher {
    pub fn new(log_recorder: LogRecorder, docker_watcher: Arc<DockerWatcher>) -> Self {
        // instantiate the process manager
        let process_manager = Arc::new(RwLock::new(ProcessManager::new(
            log_recorder.clone(),
            docker_watcher,
        )));

        EbpfWatcher {
            ebpf_initialized: Arc::new(Mutex::new(false)),
            trigger_processor: TriggerProcessor::new(Arc::clone(&process_manager)),
            process_manager,
        }
    }

    pub async fn start_ebpf(self: &Arc<Self>) -> Result<()> {
        println!("Starting ebpf");
        let mut initialized = self.ebpf_initialized.lock().await;
        if !*initialized {
            Arc::clone(self).initialize_ebpf()?;
            *initialized = true;
        }
        Ok(())
    }

    pub async fn start_process_polling(
        self: &Arc<Self>,
        process_polling_interval_ms: u64,
    ) -> Result<()> {
        info!(
            "Initializing process polling with interval {}ms",
            process_polling_interval_ms
        );
        let watcher = Arc::clone(self);
        let interval = std::time::Duration::from_millis(process_polling_interval_ms);

        tokio::spawn(async move {
            println!("Starting process polling loop");
            let mut system = sysinfo::System::new_all();
            let mut known_processes: HashSet<u32> = HashSet::new();

            loop {
                system.refresh_processes(ProcessesToUpdate::All, false);
                let mut current_processes = HashSet::new();

                // Check for new processes (started)
                for (pid, process) in system.processes() {
                    let pid_u32 = pid.as_u32();
                    current_processes.insert(pid_u32);

                    if !known_processes.contains(&pid_u32) {
                        let mut argv: Vec<String> = process
                            .cmd()
                            .iter()
                            .map(|arg| arg.to_string_lossy().to_string())
                            .collect();

                        if argv.is_empty() {
                            argv = get_process_argv(pid_u32 as i32);
                        }

                        // New process detected
                        let start_trigger = ProcessStartTrigger::from_name_and_args(
                            pid_u32 as usize,
                            process.parent().map(|p| p.as_u32()).unwrap_or(0) as usize,
                            <&str>::try_from(process.name()).unwrap_or("unknown"),
                            &argv,
                        );

                        if let Err(e) = watcher
                            .process_triggers(vec![Trigger::ProcessStart(start_trigger)])
                            .await
                        {
                            error!("Failed to process start trigger: {}", e);
                        }
                    }
                }

                // Check for ended processes
                for &old_pid in &known_processes {
                    if !current_processes.contains(&old_pid) {
                        info!("Process ended - PID: {}", old_pid);
                        // Process ended
                        let end_trigger = ProcessEndTrigger {
                            pid: old_pid as usize,
                            finished_at: Default::default(),
                            exit_reason: Some(tracer_ebpf::ebpf_trigger::ExitReason::success()),
                        };
                        if let Err(e) = watcher
                            .process_triggers(vec![Trigger::ProcessEnd(end_trigger)])
                            .await
                        {
                            error!("Failed to process end trigger for PID {}: {}", old_pid, e);
                        }
                    }
                }

                known_processes = current_processes;
                tokio::time::sleep(interval).await;
            }
        });

        Ok(())
    }

    fn initialize_ebpf(self: Arc<Self>) -> Result<(), Error> {
        info!("Initializing eBPF monitoring");
        // Use unbounded channel for cross-runtime compatibility
        let (tx, rx) = mpsc::unbounded_channel::<Trigger>();

        // Start the eBPF event processing
        info!("Starting eBPF event processing");
        match start_processing_events(tx) {
            Ok(_) => {
                info!("eBPF event processing started successfully");
            }
            Err(e) => {
                error!("Failed to start eBPF event processing: {}", e);
                return Err(e);
            }
        }

        // Start the event processing loop
        let watcher = Arc::clone(&self);
        let task = tokio::spawn(async move {
            info!("Starting eBPF event processing loop");
            if let Err(e) = watcher.process_trigger_loop(rx).await {
                error!("eBPF process trigger loop failed: {:?}", e);
            }
        });

        // Store the task handle in the state
        match tokio::runtime::Handle::try_current() {
            Ok(_) => {
                tokio::spawn(async move {
                    let process_manager = self.process_manager.write().await;
                    process_manager.set_ebpf_task(task).await;
                });
                info!("eBPF monitoring task initialized successfully");
            }
            Err(_) => {
                error!("Failed to initialize eBPF monitoring task - not in a tokio runtime");
                return Err(Error::msg("Failed to initialize eBPF monitoring task"));
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

        // Create the directory if it doesn't exist
        let log_dir = Path::new("/tmp/tracer");
        if !log_dir.exists() {
            if let Err(e) = fs::create_dir_all(log_dir) {
                error!("Failed to create log directory: {}", e);
            }
        }

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

        // Process out of memory triggers
        self.trigger_processor
            .process_out_of_memory_triggers(out_of_memory_triggers)
            .await;

        // Process end triggers
        self.trigger_processor
            .process_process_end_triggers(process_end_triggers)
            .await?;

        // Process start triggers
        self.trigger_processor
            .process_process_start_triggers(process_start_triggers)
            .await?;

        Ok(())
    }

    pub async fn poll_process_metrics(&self) -> Result<()> {
        self.process_manager
            .write()
            .await
            .poll_process_metrics()
            .await
    }

    pub async fn get_monitored_processes(&self) -> HashSet<String> {
        self.process_manager
            .write()
            .await
            .get_monitored_processes()
            .await
    }

    pub async fn get_matched_tasks(&self) -> HashSet<String> {
        self.process_manager
            .read()
            .await
            .get_matched_tasks()
            .await
    }
}
