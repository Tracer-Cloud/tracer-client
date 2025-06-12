use crate::common::recorder::LogRecorder;
use crate::common::target_process::manager::TargetManager;
use crate::common::target_process::Target;
use crate::extracts::ebpf_watcher::handler::trigger::trigger_processor::TriggerProcessor;
use crate::extracts::process::manager::ProcessManager;
use crate::extracts::process::process_utils::get_process_argv;
use anyhow::{Error, Result};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};
use tracer_ebpf::{
    subscribe, EbpfEvent, EventListener, EventPayload, EventType, SchedSchedProcessExecPayload,
    SchedSchedProcessExitPayload,
};
use tracing::{debug, error};

/// Event listener that forwards events to the trigger processor
struct EbpfEventListener {
    event_sender: mpsc::UnboundedSender<EbpfEvent<EventPayload>>,
}

impl EventListener for EbpfEventListener {
    fn on_event(&self, event: EbpfEvent<EventPayload>) {
        match self.event_sender.send(event) {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to send event: {}", e);
            }
        }
    }
}

/// Watches system processes and records events related to them
pub struct EbpfWatcher {
    ebpf: once_cell::sync::OnceCell<()>, // not tokio, because ebpf initialisation is sync
    process_manager: Arc<RwLock<ProcessManager>>,
    trigger_processor: Arc<TriggerProcessor>,
    event_sender: mpsc::UnboundedSender<EbpfEvent<EventPayload>>,
    // here will go the file manager for dataset recognition operations
}

impl EbpfWatcher {
    pub fn new(target_manager: TargetManager, log_recorder: LogRecorder) -> Self {
        // instantiate the process manager
        let process_manager = Arc::new(RwLock::new(ProcessManager::new(
            target_manager.clone(),
            log_recorder.clone(),
        )));

        let (tx, rx) = mpsc::unbounded_channel::<EbpfEvent<EventPayload>>();

        let watcher = EbpfWatcher {
            ebpf: once_cell::sync::OnceCell::new(),
            trigger_processor: Arc::new(TriggerProcessor::new(Arc::clone(&process_manager))),
            process_manager,
            event_sender: tx,
        };

        // Spawn the event processing task
        tokio::spawn(Self::process_event_loop(
            Arc::clone(&watcher.trigger_processor),
            rx,
        ));

        watcher
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
        // Only initialize eBPF once - OnceCell ensures this
        let self_clone = Arc::clone(self);
        let result = self_clone
            .ebpf
            .get_or_try_init(|| Arc::clone(&self_clone).initialize_ebpf());

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub async fn start_process_polling(
        self: &Arc<Self>,
        process_polling_interval_ms: u64,
    ) -> Result<()> {
        let watcher = Arc::clone(self);
        let interval = std::time::Duration::from_millis(process_polling_interval_ms);

        tokio::spawn(async move {
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

                        if argv.is_empty() {
                            argv = get_process_argv(pid_u32 as i32);
                        }

                        // Create a synthetic process exec event for polling-detected processes
                        let exec_event = EbpfEvent::<EventPayload> {
                            header: tracer_ebpf::EventHeader {
                                event_id: 0,
                                event_type: EventType::SchedSchedProcessExec,
                                timestamp_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                                    as u64,
                                pid: pid_u32,
                                ppid: process.parent().map(|p| p.as_u32()).unwrap_or(0),
                                upid: pid_u32 as u64,
                                uppid: process.parent().map(|p| p.as_u32()).unwrap_or(0) as u64,
                                comm: process.name().to_string(),
                            },
                            payload: EventPayload::SchedSchedProcessExec(
                                SchedSchedProcessExecPayload { argv },
                            ),
                        };

                        if let Err(e) = EbpfWatcher::process_events(
                            &watcher.trigger_processor,
                            vec![exec_event],
                        )
                        .await
                        {
                            error!("Failed to process synthetic exec event: {}", e);
                        }
                    }
                }

                // Check for ended processes
                for &old_pid in &known_processes {
                    if !current_processes.contains(&old_pid) {
                        // Create a synthetic process exit event
                        let exit_event = EbpfEvent::<EventPayload> {
                            header: tracer_ebpf::EventHeader {
                                event_id: 0,
                                event_type: EventType::SchedSchedProcessExit,
                                timestamp_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                                    as u64,
                                pid: old_pid,
                                ppid: 0,
                                upid: old_pid as u64,
                                uppid: 0,
                                comm: String::new(),
                            },
                            payload: EventPayload::SchedSchedProcessExit(
                                SchedSchedProcessExitPayload { exit_code: 0 },
                            ),
                        };

                        if let Err(e) = EbpfWatcher::process_events(
                            &watcher.trigger_processor,
                            vec![exit_event],
                        )
                        .await
                        {
                            error!("Failed to process synthetic exit event: {}", e);
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
        // Create event listener that uses the existing channel
        let listener = EbpfEventListener {
            event_sender: self.event_sender.clone(),
        };

        // Start the eBPF event processing
        subscribe(listener)?;

        // Keep eBPF alive in a background thread, matching example.rs approach
        std::thread::spawn(|| {
            let should_exit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

            // No signal handler setup since this is integrated into a larger system
            // The eBPF will be shut down when the process exits

            // Main loop - keep the eBPF system alive
            while !should_exit.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        });

        Ok(())
    }

    /// Static method for processing a batch of events
    pub async fn process_event_loop(
        trigger_processor: Arc<TriggerProcessor>,
        mut rx: mpsc::UnboundedReceiver<EbpfEvent<EventPayload>>,
    ) -> Result<()> {
        let mut buffer: Vec<EbpfEvent<EventPayload>> = Vec::with_capacity(64);
        let mut last_flush = std::time::Instant::now();
        const FLUSH_INTERVAL: Duration = Duration::from_millis(100);

        loop {
            tokio::select! {
                // Receive events
                event = rx.recv() => {
                    match event {
                        Some(event) => {
                            buffer.push(event);

                            // Flush if buffer is full or enough time has passed
                            if buffer.len() >= 64 || last_flush.elapsed() >= FLUSH_INTERVAL {
                                if let Err(e) = Self::process_events(&trigger_processor, std::mem::take(&mut buffer)).await {
                                    error!("Failed to process events: {}", e);
                                }
                                last_flush = std::time::Instant::now();
                            }
                        }
                        None => {
                            debug!("Event channel closed, stopping event processing loop");
                            break;
                        }
                    }
                }

                // Periodic flush
                _ = sleep(FLUSH_INTERVAL) => {
                    if !buffer.is_empty() && last_flush.elapsed() >= FLUSH_INTERVAL {
                        if let Err(e) = Self::process_events(&trigger_processor, std::mem::take(&mut buffer)).await {
                            error!("Failed to process events: {}", e);
                        }
                        last_flush = std::time::Instant::now();
                    }
                }
            }
        }

        // Process any remaining events
        if !buffer.is_empty() {
            if let Err(e) = Self::process_events(&trigger_processor, buffer).await {
                error!("Failed to process final events: {}", e);
            }
        }

        Ok(())
    }

    /// Processes a batch of events using the trigger processor
    pub async fn process_events(
        trigger_processor: &Arc<TriggerProcessor>,
        events: Vec<EbpfEvent<EventPayload>>,
    ) -> Result<()> {
        // Process events in the correct order
        trigger_processor.process_events(events).await?;

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
