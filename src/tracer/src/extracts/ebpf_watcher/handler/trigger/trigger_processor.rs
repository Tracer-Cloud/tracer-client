use crate::extracts::process::manager::ProcessManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracer_ebpf::ebpf_trigger::{OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger};
use tracing::{debug, info};

pub struct TriggerProcessor {
    process_manager: Arc<RwLock<ProcessManager>>,
}

impl TriggerProcessor {
    pub fn new(process_manager: Arc<RwLock<ProcessManager>>) -> Self {
        info!("Initializing TriggerProcessor");
        Self { process_manager }
    }

    pub async fn process_out_of_memory_triggers(
        &self,
        out_of_memory_triggers: Vec<OutOfMemoryTrigger>,
    ) {
        if !out_of_memory_triggers.is_empty() {
            info!("Processing {} out of memory triggers", out_of_memory_triggers.len());
            for trigger in &out_of_memory_triggers {
                debug!("Processing OOM trigger for PID: {}", trigger.pid);
            }
            self.process_manager
                .write()
                .await
                .handle_out_of_memory_signals(out_of_memory_triggers)
                .await;
        }
    }

    pub async fn process_process_end_triggers(
        &self,
        mut process_end_triggers: Vec<ProcessEndTrigger>,
    ) -> Result<()> {
        if !process_end_triggers.is_empty() {
            info!("Processing {} process end triggers", process_end_triggers.len());
            for trigger in &process_end_triggers {
                debug!("Processing end trigger for PID: {}", trigger.pid);
            }

            self.process_manager
                .write()
                .await
                .handle_out_of_memory_terminations(&mut process_end_triggers)
                .await;

            self.process_manager
                .write()
                .await
                .handle_process_terminations(process_end_triggers)
                .await?;
        }

        Ok(())
    }

    pub async fn process_process_start_triggers(
        &self,
        process_start_triggers: Vec<ProcessStartTrigger>,
    ) -> Result<()> {
        if !process_start_triggers.is_empty() {
            info!("Processing {} process start triggers", process_start_triggers.len());
            for trigger in &process_start_triggers {
                debug!(
                    "Processing start trigger - PID: {}, Name: {}, Parent PID: {}",
                    trigger.pid, trigger.comm, trigger.ppid
                );
            }
            self.process_manager
                .write()
                .await
                .handle_process_starts(process_start_triggers)
                .await?;
        }

        Ok(())
    }
}
