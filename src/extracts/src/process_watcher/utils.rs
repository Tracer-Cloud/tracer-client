use super::ProcessState;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracer_common::types::trigger::{ExitReason, FinishTrigger, OomTrigger};
use tracing::debug;

/// Enriches finish triggers with OOM reason if they were OOM victims
pub async fn handle_oom_terminations(
    state: &Arc<RwLock<ProcessState>>,
    finish_triggers: &mut [FinishTrigger],
) {
    let mut state = state.write().await;

    for finish in finish_triggers.iter_mut() {
        if state.oom_victims.remove(&finish.pid).is_some() {
            finish.exit_reason = Some(ExitReason::OomKilled);
            debug!("Marked PID {} as OOM-killed", finish.pid);
        }
    }
}

pub async fn handle_oom_signals(
    state: &Arc<RwLock<ProcessState>>,
    triggers: Vec<OomTrigger>,
) -> HashMap<usize, OomTrigger> {
    let mut victims = HashMap::new();
    let mut state = state.write().await;

    for oom in triggers {
        let is_related = state.processes.contains_key(&oom.pid)
            || state.processes.values().any(|p| p.ppid == oom.pid);

        if is_related {
            debug!("Tracking OOM for relevant pid {}", oom.pid);
            victims.insert(oom.pid, oom.clone());
            state.oom_victims.insert(oom.pid, oom);
        } else {
            debug!("Ignoring unrelated OOM for pid {}", oom.pid);
        }
    }

    victims
}
