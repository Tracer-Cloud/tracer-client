use crate::process_watcher::handler::process::process_manager::ProcessManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracer_common::types::ebpf_trigger::{
    OutOfMemoryTrigger, ProcessEndTrigger, ProcessStartTrigger,
};
use tracing::debug;

pub struct TriggerProcessor {
    process_manager: Arc<RwLock<ProcessManager>>,
}

impl TriggerProcessor {
    pub fn new(process_manager: Arc<RwLock<ProcessManager>>) -> Self {
        Self { process_manager }
    }

    pub async fn process_out_of_memory_triggers(
        &self,
        out_of_memory_triggers: Vec<OutOfMemoryTrigger>,
    ) {
        if !out_of_memory_triggers.is_empty() {
            debug!("Processing {} oom processes", out_of_memory_triggers.len());
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
            debug!(
                "Processing {} finishing processes",
                process_end_triggers.len()
            );

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
            debug!(
                "Processing {} creating processes",
                process_start_triggers.len()
            );
            self.process_manager
                .write()
                .await
                .handle_process_starts(process_start_triggers)
                .await?;
        }

        Ok(())
    }
}
