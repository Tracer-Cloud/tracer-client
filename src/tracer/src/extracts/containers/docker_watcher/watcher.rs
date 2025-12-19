use crate::extracts::containers::docker_watcher::event::{
    ContainerEvent, ContainerId, ContainerState,
};
use crate::process_identification::recorder::EventDispatcher;
use crate::process_identification::types::event::attributes::EventAttributes;
use crate::process_identification::types::event::ProcessStatus;
use anyhow::Result;
use bollard::models::EventMessage;
use bollard::query_parameters::{EventsOptionsBuilder, InspectContainerOptions};
use bollard::Docker;
use chrono::{DateTime, TimeZone, Utc};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracer_ebpf::ebpf_trigger::exit_code_explanation;
#[derive(Clone)]
pub struct DockerWatcher {
    docker: Option<Docker>,
    recorder: EventDispatcher,
    container_state: Arc<RwLock<HashMap<ContainerId, ContainerEvent>>>, // Keyed by container ID
}

impl DockerWatcher {
    pub fn new(recorder: EventDispatcher) -> Self {
        let docker = Docker::connect_with_unix_defaults().ok(); // returns Option<Docker>

        if docker.is_none() {
            tracing::warn!("Docker not available - container events will not be captured.");
        }
        let container_state = Arc::new(RwLock::new(HashMap::new()));

        Self {
            docker,
            recorder,
            container_state,
        }
    }

    pub fn new_lazy(recorder: EventDispatcher) -> Self {
        let container_state = Arc::new(RwLock::new(HashMap::new()));

        Self {
            docker: None,
            recorder,
            container_state,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let docker = if self.docker.is_none() {
            let docker = Docker::connect_with_unix_defaults().ok();
            if docker.is_none() {
                tracing::warn!("Docker not available - container events will not be captured.");
            }
            docker
        } else {
            self.docker.clone()
        };

        if let Some(ref docker) = docker {
            // Scan existing containers first
            if let Err(e) = self.scan_existing_containers(docker).await {
                tracing::warn!("Failed to scan existing containers: {:?}", e);
            }

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

                        // we will define if it's a container execution or a container termination
                        let mut process_status = ProcessStatus::ContainerExecution;
                        match container_event.state {
                            ContainerState::Started => {
                                // the process_status is set already to ContainerExecution, so we don't need to change it here
                                state.insert(container_id, container_event.clone());
                            }
                            ContainerState::Exited { .. } | ContainerState::Died => {
                                state.remove(&container_id);
                                process_status = ProcessStatus::ContainerTermination;
                            }
                        }
                        // Log the container event
                        if let Err(e) = recorder
                            .log_with_metadata(
                                process_status,
                                "[container event]".to_string(),
                                Some(EventAttributes::ContainerEvents(
                                    container_event.clone().into(),
                                )),
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

    async fn scan_existing_containers(&self, docker: &Docker) -> Result<()> {
        let containers = docker
            .list_containers(None::<bollard::query_parameters::ListContainersOptions>)
            .await?;

        for container in containers {
            let Some(ref id) = container.id else { continue };

            let Ok(inspect) = docker
                .inspect_container(id, None::<InspectContainerOptions>)
                .await
            else {
                continue;
            };

            if !matches!(inspect.state.as_ref().and_then(|s| s.running), Some(true)) {
                continue;
            }

            let timestamp = inspect
                .state
                .as_ref()
                .and_then(|s| s.started_at.as_ref()?.parse::<DateTime<Utc>>().ok())
                .unwrap_or_else(Utc::now);

            let Some(event) = Self::inspect_to_event(inspect, timestamp, ContainerState::Started)
            else {
                continue;
            };

            self.container_state
                .write()
                .await
                .insert(ContainerId(event.id.clone()), event.clone());

            if let Err(e) = self
                .recorder
                .log_with_metadata(
                    ProcessStatus::ContainerExecution,
                    "[existing container]".to_string(),
                    Some(EventAttributes::ContainerEvents(event.into())),
                    Some(timestamp),
                )
                .await
            {
                tracing::error!("Failed to log existing container: {:?}", e);
            }
        }

        tracing::info!("Scanned existing containers");
        Ok(())
    }

    fn get_container_environment_variable(env_vars: &[String], name: &str) -> Option<String> {
        env_vars
            .iter()
            .find(|v| v.starts_with(&format!("{}=", name)))
            .and_then(|v| v.split('=').nth(1).map(String::from))
    }

    fn inspect_to_event(
        inspect: bollard::models::ContainerInspectResponse,
        timestamp: DateTime<Utc>,
        state: ContainerState,
    ) -> Option<ContainerEvent> {
        let env_vars = inspect.config.as_ref()?.env.clone().unwrap_or_default();

        Some(ContainerEvent {
            id: inspect.id?,
            name: inspect
                .name
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_string(),
            image: inspect
                .config
                .as_ref()
                .and_then(|cfg| cfg.image.clone())
                .unwrap_or_default(),
            ip: inspect.network_settings.and_then(|net| {
                net.ip_address
                    .or_else(|| net.networks?.values().next()?.ip_address.clone())
            }),
            labels: inspect
                .config
                .and_then(|cfg| cfg.labels)
                .unwrap_or_default(),
            timestamp,
            state,
            trace_id: Self::get_container_environment_variable(&env_vars, "TRACER_TRACE_ID"),
            job_id: Self::get_container_environment_variable(&env_vars, "AWS_BATCH_JOB_ID"),
            environment_variables: env_vars,
        })
    }

    async fn process_event(docker: &Docker, event: EventMessage) -> Option<ContainerEvent> {
        let id = event.actor.as_ref()?.id.as_ref()?;
        let action = event.action.as_deref()?;
        let time = Utc
            .timestamp_opt(event.time.unwrap_or_default(), 0)
            .single()?;

        let state = match action {
            "start" => ContainerState::Started,
            "die" => {
                let exit_code = event
                    .actor
                    .as_ref()?
                    .attributes
                    .as_ref()?
                    .get("exitCode")?
                    .parse()
                    .ok()
                    .unwrap_or(-1);
                let reason = exit_code_explanation(exit_code);
                ContainerState::Exited { exit_code, reason }
            }
            "destroy" => ContainerState::Died,
            _ => return None,
        };

        let inspect = docker
            .inspect_container(id, None::<InspectContainerOptions>)
            .await
            .ok()?;
        Self::inspect_to_event(inspect, time, state)
    }

    pub async fn get_container_event(&self, id: &str) -> Option<ContainerEvent> {
        let container_id = ContainerId(id.to_string());
        let state = self.container_state.read().await;

        // Log all keys (container IDs) currently stored
        tracing::info!(
            "Looking for container ID: {:?} | Currently stored IDs: {:?}",
            container_id,
            state.keys().collect::<Vec<&ContainerId>>()
        );

        state.get(&container_id).cloned()
    }
}
