use crate::extracts::process::process_manager::state::StateManager;
use std::collections::HashMap;
use tracer_ebpf::ebpf_trigger::{ExitReason, OutOfMemoryTrigger, ProcessEndTrigger};
use tracing::debug;

/// Handles out-of-memory related process events
pub struct OomHandler;

impl OomHandler {
    /// Enriches finish triggers with OOM reason if they were OOM victims
    pub async fn handle_out_of_memory_terminations(
        state_manager: &StateManager,
        finish_triggers: &mut [ProcessEndTrigger],
    ) {
        for finish in finish_triggers.iter_mut() {
            if state_manager
                .remove_out_of_memory_victim(&finish.pid)
                .await
                .is_some()
            {
                finish.exit_reason = Some(ExitReason::out_of_memory_killed());
                debug!("Marked PID {} as OOM-killed", finish.pid);
            }
        }
    }

    /// Handles out-of-memory signals and tracks relevant victims
    pub async fn handle_out_of_memory_signals(
        state_manager: &StateManager,
        triggers: Vec<OutOfMemoryTrigger>,
    ) -> HashMap<usize, OutOfMemoryTrigger> {
        let mut victims = HashMap::new();

        for oom in triggers {
            let state = state_manager.get_state().await;
            let processes = state.get_processes();
            let is_related =
                processes.contains_key(&oom.pid) || processes.values().any(|p| p.ppid == oom.pid);

            if is_related {
                debug!("Tracking OOM for relevant pid {}", oom.pid);
                victims.insert(oom.pid, oom.clone());
                drop(state); // Release the read lock before acquiring write lock
                state_manager
                    .insert_out_of_memory_victim(oom.pid, oom)
                    .await;
            } else {
                debug!("Ignoring unrelated OOM for pid {}", oom.pid);
            }
        }

        victims
    }
}
