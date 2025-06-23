use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;

//use crate::extracts::containers::docker_watcher::{ContainerEvent, ContainerState};
use anyhow::Result;
use bollard::models::EventMessage;
use bollard::query_parameters::EventsOptionsBuilder;
use bollard::query_parameters::InspectContainerOptions;

use bollard::Docker;
//API_DEFAULT_VERSION
use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum ContainerState {
    Started,
    Exited { exit_code: i64 },
    Died,
}

#[derive(Debug, Clone)]
pub struct ContainerEvent {
    pub id: String,
    pub name: String,
    pub image: String,
    pub ip: Option<String>,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
    pub state: ContainerState,
}

pub async fn start_docker_watcher(tx: UnboundedSender<ContainerEvent>) -> Result<()> {
    let docker = Docker::connect_with_unix_defaults()?;

    let filters = HashMap::from_iter([("type", vec!["container".to_string()])]);
    let events_options = EventsOptionsBuilder::default().filters(&filters).build();
    let mut events_stream = docker.events(Some(events_options));

    tokio::spawn(async move {
        while let Some(Ok(event)) = events_stream.next().await {
            if let Some(container_event) = process_event(&docker, event).await {
                let _ = tx.send(container_event);
            }
        }
    });

    Ok(())
}

async fn process_event(docker: &Docker, event: EventMessage) -> Option<ContainerEvent> {
    let id = event.actor.as_ref().map(|action| action.id.clone())??;
    let action = event.action.as_deref()?;
    let time = Utc
        .timestamp_opt(event.time.unwrap_or_default() as i64, 0)
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
        "die" => ContainerState::Exited {
            exit_code: event
                .actor
                .and_then(|a| a.attributes)
                .and_then(|attrs| attrs.get("exitCode")?.parse().ok())
                .unwrap_or(-1),
        },
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
