use anyhow::Result;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracer_common::types::trigger::Trigger;
use tracer_ebpf_libbpf::start_processing_events;
use tracing::{debug, error};

/// Manages eBPF program lifecycle and event processing
pub struct EbpfManager {
    ebpf: Arc<OnceCell<()>>,
    state: Arc<RwLock<EbpfState>>,
}

struct EbpfState {
    ebpf_task: Option<JoinHandle<()>>,
}

impl EbpfManager {
    pub fn new() -> Self {
        EbpfManager {
            ebpf: Arc::new(OnceCell::new()),
            state: Arc::new(RwLock::new(EbpfState { ebpf_task: None })),
        }
    }

    pub async fn start_ebpf(self: &Arc<Self>) -> Result<()> {
        // Check if eBPF is already initialized
        if self.ebpf.get().is_some() {
            debug!("eBPF already initialized, skipping");
            return Ok(()); // Already initialized
        }

        debug!("Starting eBPF event processing...");

        // Initialize eBPF components
        let (tx, mut rx) = mpsc::unbounded_channel::<Trigger>();

        // Start the eBPF event processing
        debug!("Calling start_processing_events...");
        if let Err(e) = start_processing_events(tx) {
            error!("Failed to start eBPF processing: {:?}", e);
            return Err(e);
        }
        debug!("start_processing_events completed successfully");

        // Mark eBPF as initialized
        if self.ebpf.set(()).is_err() {
            // Another thread already initialized it, that's fine
            debug!("eBPF was already initialized by another thread");
            return Ok(());
        }

        // Clone self for the task
        let self_clone = Arc::clone(self);

        let ebpf_task = tokio::spawn(async move {
            debug!("eBPF event processing loop started, waiting for triggers...");
            let mut buffer = Vec::new();
            loop {
                match rx.recv().await {
                    Some(event) => {
                        println!("Received eBPF trigger: {:?}", event);
                        buffer.push(event);
                        // Try to receive more events non-blockingly (up to 99 more)
                        let mut count = 1;
                        while let Ok(Some(event)) =
                            tokio::time::timeout(std::time::Duration::from_millis(10), rx.recv())
                                .await
                        {
                            println!("Received additional eBPF trigger: {:?}", event);
                            buffer.push(event);
                            count += 1;
                            if count >= 100 {
                                break;
                            }
                        }

                        // Process all events
                        let triggers = std::mem::take(&mut buffer);
                        println!(
                            "process_trigger_loop: Processing {} triggers",
                            triggers.len()
                        );

                        if let Err(e) = self_clone.handle_incoming_triggers(triggers).await {
                            println!("Failed to process triggers: {}", e);
                        }
                    }
                    None => {
                        error!("Event channel closed, exiting process loop");
                        return;
                    }
                }
            }
        });

        // Store the task handle using async write
        let mut state = self.state.write().await;
        state.ebpf_task = Some(ebpf_task);

        println!("eBPF initialization completed successfully");
        Ok(())
    }

    async fn handle_incoming_triggers(&self, triggers: Vec<Trigger>) -> Result<()> {
        // TODO: Implement trigger handling logic
        Ok(())
    }
}
