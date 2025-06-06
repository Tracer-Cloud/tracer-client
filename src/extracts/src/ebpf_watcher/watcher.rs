use crate::ebpf_watcher::handler::process::process_manager::ProcessManager;
use crate::ebpf_watcher::handler::process::process_utils::get_process_argv;
use crate::ebpf_watcher::handler::trigger::trigger_processor::TriggerProcessor;
use anyhow::{Error, Result};
use chrono::Utc;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracer_common::recorder::LogRecorder;
use tracer_common::target_process::manager::TargetManager;
use tracer_common::target_process::Target;
use tracer_common::types::ebpf_trigger::{
    OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger, Trigger,
};
use tracer_ebpf::binding::start_processing_events;
use tracing::{debug, error};

/// Watches system processes and records events related to them
pub struct EbpfWatcher {
    ebpf: once_cell::sync::OnceCell<()>, // not tokio, because ebpf initialisation is sync
    process_manager: Arc<RwLock<ProcessManager>>,
    trigger_processor: TriggerProcessor,
    // here will go the file manager for dataset recognition operations
}

impl EbpfWatcher {
    pub fn new(target_manager: TargetManager, log_recorder: LogRecorder) -> Self {
        // instantiate the process manager
        let process_manager = Arc::new(RwLock::new(ProcessManager::new(
            target_manager.clone(),
            log_recorder.clone(),
        )));

        EbpfWatcher {
            ebpf: once_cell::sync::OnceCell::new(),
            trigger_processor: TriggerProcessor::new(Arc::clone(&process_manager)),
            process_manager,
        }
    }

    pub async fn update_targets(self: &Arc<Self>, targets: Vec<Target>) -> Result<()> {
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

    pub async fn start_process_polling(
        self: &Arc<Self>,
        process_polling_interval_ms: u64,
    ) -> Result<()> {
        println!("Starting process polling");
        let watcher = Arc::clone(self);
        let interval = std::time::Duration::from_millis(process_polling_interval_ms);

        tokio::spawn(async move {
            println!("Starting process polling loop");
            let mut system = sysinfo::System::new_all();
            let mut known_processes: HashSet<u32> = HashSet::new();

            loop {
                system.refresh_processes();
                let mut current_processes = HashSet::new();

                // Check for new processes (started)
                for (pid, process) in system.processes() {
                    let pid_u32 = pid.as_u32();
                    current_processes.insert(pid_u32);

                    if !known_processes.contains(&pid_u32) {
                        let mut argv = process.cmd().to_vec();

                        #[cfg(target_os = "macos")]
                        {
                            argv = get_process_argv(pid_u32 as i32);
                        }

                        // New process detected
                        let start_trigger = ProcessStartTrigger {
                            pid: pid_u32 as usize,
                            ppid: process.parent().map(|p| p.as_u32()).unwrap_or(0) as usize,
                            comm: process.name().to_string(),
                            argv,
                            file_name: "".to_string(),
                            started_at: Utc::now(),
                        };

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
                        // Process ended
                        let end_trigger = ProcessEndTrigger {
                            pid: old_pid as usize,
                            finished_at: Default::default(),
                            exit_reason: None,
                        };
                        if let Err(e) = watcher
                            .process_triggers(vec![Trigger::ProcessEnd(end_trigger)])
                            .await
                        {
                            error!("Failed to process end trigger: {}", e);
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

        // Store the task handle in the stateAdd commentMore actions
        match tokio::runtime::Handle::try_current() {
            Ok(_) => {
                tokio::spawn(async move {
                    let mut process_manager = self.process_manager.write().await;
                    process_manager.set_ebpf_task(task).await;
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

    pub async fn get_n_monitored_processes(&self, n: usize) -> HashSet<String> {
        self.process_manager
            .write()
            .await
            .get_n_monitored_processes(n)
            .await
    }

    pub async fn get_number_of_monitored_processes(&self) -> usize {
        self.process_manager
            .write()
            .await
            .get_number_of_monitored_processes()
            .await
    }
}
