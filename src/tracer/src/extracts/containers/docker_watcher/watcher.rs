use crate::common::recorder::LogRecorder;
use crate::extracts::containers::docker_watcher::event::{ContainerEvent, ContainerState};
use anyhow::Result;
use bollard::models::EventMessage;
use bollard::query_parameters::{EventsOptionsBuilder, InspectContainerOptions};
use bollard::Docker;
use chrono::{TimeZone, Utc};
use futures_util::StreamExt;
use std::collections::HashMap;
use tracer_ebpf::ebpf_trigger::ExitReason;

pub struct DockerWatcher {
    docker: Docker,
    recorder: LogRecorder,
}

impl DockerWatcher {
    pub fn new(recorder: LogRecorder) -> Result<Self> {
        let docker = Docker::connect_with_unix_defaults()?;
        Ok(Self { docker, recorder })
    }

    pub async fn start(self) -> Result<()> {
        let filters = HashMap::from_iter([("type", vec!["container".to_string()])]);
        let events_options = EventsOptionsBuilder::default().filters(&filters).build();
        let mut events_stream = self.docker.events(Some(events_options));

        let docker = self.docker.clone();
        let recorder = self.recorder.clone();

        tokio::spawn(async move {
            while let Some(Ok(event)) = events_stream.next().await {
                if let Some(container_event) = Self::process_event(&docker, event).await {
                    tracing::debug!("Container event: {:?}", container_event);

                    if let Err(e) = recorder
                        .log(
                            crate::common::types::event::ProcessStatus::ContainerExecution,
                            format!(
                                "[container] {} - {:?}",
                                container_event.name, container_event.state
                            ),
                            Some(
                                crate::common::types::event::attributes::EventAttributes::ContainerEvents(
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
                let reason = ExitReason::from_exit_code(exit_code).explanation();
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
}
