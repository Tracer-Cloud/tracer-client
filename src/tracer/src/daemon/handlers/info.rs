use crate::client::TracerClient;
use crate::daemon::state::DaemonState;
use crate::daemon::structs::{InfoResponse, InnerInfoResponse, OpenTelemetryStatus};
use crate::opentelemetry::collector::OtelCollector;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

pub const INFO_ENDPOINT: &str = "/info";

pub async fn info(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;
    let response = get_info_response(&guard).await;

    Ok(Json(response))
}

pub async fn get_info_response(client: &TracerClient) -> InfoResponse {
    let pipeline = client.get_run_metadata().read().await.clone();
    let mut response_inner = InnerInfoResponse::try_from(pipeline).ok();

    if let Some(ref mut inner) = response_inner {
        inner.opentelemetry_status = get_opentelemetry_status().await;
    }

    let processes = client.process_watcher.get_monitored_processes().await;

    let tasks = client.process_watcher.get_matched_tasks().await;

    InfoResponse::new(response_inner, processes, tasks)
}

async fn get_opentelemetry_status() -> Option<OpenTelemetryStatus> {
    match OtelCollector::new() {
        Ok(collector) => {
            let enabled = collector.is_running();
            let version = collector.get_version();
            let pid = if enabled {
                let pid_file = crate::utils::workdir::TRACER_WORK_DIR.resolve("otelcol.pid");
                if pid_file.exists() {
                    std::fs::read_to_string(&pid_file)
                        .ok()
                        .and_then(|content| content.trim().parse::<u32>().ok())
                } else {
                    None
                }
            } else {
                None
            };

            Some(OpenTelemetryStatus {
                enabled,
                version,
                pid,
                endpoint: Some("opensearch".to_string()), // Default endpoint
            })
        }
        Err(_) => None,
    }
}
