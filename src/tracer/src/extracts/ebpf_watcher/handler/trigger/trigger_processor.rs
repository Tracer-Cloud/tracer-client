use crate::extracts::process::manager::ProcessManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracer_ebpf::{EbpfEvent, EventPayload, EventType, OomMarkVictimPayload, SchedSchedProcessExecPayload, SchedSchedProcessExitPayload};
use tracing::debug;

pub struct TriggerProcessor {
    process_manager: Arc<RwLock<ProcessManager>>,
}

impl TriggerProcessor {
    pub fn new(process_manager: Arc<RwLock<ProcessManager>>) -> Self {
        Self { process_manager }
    }

    pub async fn process_events(&self, events: Vec<EbpfEvent<EventPayload>>) -> Result<()> {
        debug!("Processing {} events", events.len());

        // Group events by type
        let mut process_exec_events = Vec::new();
        let mut process_exit_events = Vec::new();
        let mut oom_events = Vec::new();

        for event in events {
            match event.payload {
                EventPayload::SchedSchedProcessExec(_) => process_exec_events.push(event),
                EventPayload::SchedSchedProcessExit(_) => process_exit_events.push(event),
                EventPayload::OomMarkVictim(_) => oom_events.push(event),
                _ => {
                    // Ignore other event types
                }
            }
        }

        // Process each event type
        if !oom_events.is_empty() {
            self.process_out_of_memory_events(oom_events).await;
        }

        if !process_exit_events.is_empty() {
            self.process_process_exit_events(process_exit_events)
                .await?;
        }

        if !process_exec_events.is_empty() {
            self.process_process_exec_events(process_exec_events)
                .await?;
        }

        Ok(())
    }

    async fn process_out_of_memory_events(&self, oom_events: Vec<EbpfEvent<EventPayload>>) {
        debug!("Processing {} oom events", oom_events.len());

        // Convert to specific event type
        let typed_events: Vec<EbpfEvent<OomMarkVictimPayload>> = oom_events
            .into_iter()
            .filter_map(|event| {
                if event.header.event_type == EventType::OomMarkVictim {
                    if let EventPayload::OomMarkVictim(payload) = event.payload {
                        Some(EbpfEvent {
                            header: event.header,
                            payload,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        self.process_manager
            .write()
            .await
            .handle_out_of_memory_signals(typed_events)
            .await;
    }

    async fn process_process_exit_events(
        &self,
        exit_events: Vec<EbpfEvent<EventPayload>>,
    ) -> Result<()> {
        debug!("Processing {} exit events", exit_events.len());

        // Convert to specific event type
        let mut typed_events: Vec<EbpfEvent<SchedSchedProcessExitPayload>> = exit_events
            .into_iter()
            .filter_map(|event| {
                if event.header.event_type == EventType::SchedSchedProcessExit {
                    if let EventPayload::SchedSchedProcessExit(payload) = event.payload {
                        Some(EbpfEvent {
                            header: event.header,
                            payload,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        self.process_manager
            .write()
            .await
            .handle_out_of_memory_terminations(&mut typed_events)
            .await;

        self.process_manager
            .write()
            .await
            .handle_process_terminations(typed_events)
            .await?;

        Ok(())
    }

    async fn process_process_exec_events(
        &self,
        exec_events: Vec<EbpfEvent<EventPayload>>,
    ) -> Result<()> {
        debug!("Processing {} exec events", exec_events.len());

        // Convert to specific event type
        let typed_events: Vec<EbpfEvent<SchedSchedProcessExecPayload>> = exec_events
            .into_iter()
            .filter_map(|event| {
                if event.header.event_type == EventType::SchedSchedProcessExec {
                    if let EventPayload::SchedSchedProcessExec(payload) = event.payload {
                        Some(EbpfEvent {
                            header: event.header,
                            payload,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        self.process_manager
            .write()
            .await
            .handle_process_starts(typed_events)
            .await?;

        Ok(())
    }
}
