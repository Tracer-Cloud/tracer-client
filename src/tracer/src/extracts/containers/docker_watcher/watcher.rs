use crate::extracts::containers::docker_watcher::event::{
    ContainerEvent, ContainerId, ContainerState,
};
use crate::process_identification::recorder::LogRecorder;
use anyhow::Result;
use bollard::models::EventMessage;
use bollard::query_parameters::{EventsOptionsBuilder, InspectContainerOptions};
use bollard::Docker;
use chrono::{TimeZone, Utc};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracer_ebpf::ebpf_trigger::ExitReason;

#[derive(Clone)]
pub struct DockerWatcher {
    docker: Option<Docker>,
    recorder: LogRecorder,
    container_state: Arc<RwLock<HashMap<ContainerId, ContainerEvent>>>, // Keyed by container ID
}

impl DockerWatcher {
    pub fn new(recorder: LogRecorder) -> Self {
        let docker = Docker::connect_with_unix_defaults().ok(); // returns Option<Docker>

        if docker.is_none() {
            tracing::warn!("Docker not available — container events will not be captured.");
        }
        let container_state = Arc::new(RwLock::new(HashMap::new()));

        Self {
            docker,
            recorder,
            container_state,
        }
    }

    pub async fn start(&self) -> Result<()> {
        if let Some(ref docker) = self.docker {
            let filters = HashMap::from_iter([("type", vec!["container".to_string()])]);
            let events_options = EventsOptionsBuilder::default().filters(&filters).build();
            let mut events_stream = docker.events(Some(events_options));

            let docker = docker.clone();
            let recorder = self.recorder.clone();

            let container_state = Arc::clone(&self.container_state);

            tokio::spawn(async move {
                while let Some(Ok(event)) = events_stream.next().await {
                    if let Some(container_event) = Self::process_event(&docker, event).await {
                        tracing::debug!("Container event: {:?}", container_event);

                        let container_id = ContainerId(container_event.id.clone());
                        let mut state = container_state.write().await;

                        match container_event.state {
                            ContainerState::Started => {
                                state.insert(container_id, container_event.clone());
                            }
                            ContainerState::Exited { .. } | ContainerState::Died => {
                                state.remove(&container_id);
                            }
                        }
                        // Log the container event
                        if let Err(e) = recorder
                        .log(
                            crate::process_identification::types::event::ProcessStatus::ContainerExecution,
                            format!("[container] {} - {:?}",
                            container_event.name, container_event.state
                        ),
                            Some(
                                crate::process_identification::types::event::attributes::EventAttributes::ContainerEvents(
                                container_event.clone().into(),
                            ),
                        ),
                            Some(container_event.timestamp),
                        )
                        .await
                    {
                        tracing::error!("Failed to log container event: {:?}", e);
                    }
                    }
                }
            });
        }
        Ok(())
    }

    async fn process_event(docker: &Docker, event: EventMessage) -> Option<ContainerEvent> {
        let id = event.actor.as_ref().map(|action| action.id.clone())??;
        let action = event.action.as_deref()?;
        let time = Utc
            .timestamp_opt(event.time.unwrap_or_default(), 0)
            .single()?;

        let inspect = docker
            .inspect_container(&id, None::<InspectContainerOptions>)
            .await
            .ok()?;

        let name = inspect
            .name
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string();
        let image = inspect
            .config
            .as_ref()
            .and_then(|cfg| cfg.image.clone())
            .unwrap_or_default();
        let labels = inspect
            .config
            .and_then(|cfg| cfg.labels)
            .unwrap_or_default();
        let ip = inspect.network_settings.and_then(|net| {
            net.ip_address.or_else(|| {
                net.networks
                    .and_then(|m| m.values().next().and_then(|n| n.ip_address.clone()))
            })
        });

        let state = match action {
            "start" => ContainerState::Started,
            "die" => {
                let exit_code = event
                    .actor
                    .and_then(|a| a.attributes)
                    .and_then(|attrs| attrs.get("exitCode")?.parse().ok())
                    .unwrap_or(-1);
                // TODO: if the attributes contain the invoked command, add the command name to
                // the error messages when relevant
                let reason = ExitReason::from(exit_code).explanation;
                ContainerState::Exited { exit_code, reason }
            }
            "destroy" => ContainerState::Died,
            _ => return None,
        };

        Some(ContainerEvent {
            id,
            name,
            image,
            ip,
            labels,
            timestamp: time,
            state,
        })
    }

    pub async fn get_container_event(&self, id: &str) -> Option<ContainerEvent> {
        let container_id = ContainerId(id.to_string());
        let state = self.container_state.read().await;

        // Log all keys (container IDs) currently stored
        tracing::error!(
            "Looking for container ID: {:?} | Currently stored IDs: {:?}",
            container_id,
            state.keys().collect::<Vec<&ContainerId>>()
        );

        state.get(&container_id).cloned()
    }
}
