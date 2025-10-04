use crate::daemon::state::DaemonState;
use crate::daemon::structs::OpenTelemetryStatus;
use crate::opentelemetry::collector::OtelCollector;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

pub const INFO_ENDPOINT: &str = "/info";

pub async fn info(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;
    let mut pipeline_data = if let Some(client) = guard {
        // Use timeout to prevent blocking - if the client is busy, fall back to state data
        match tokio::time::timeout(std::time::Duration::from_millis(500), client.lock()).await {
            Ok(client) => client.get_pipeline_data().await,
            Err(_) => {
                // Timeout occurred, fall back to state data
                tracing::warn!(
                    "Timeout waiting for tracer client lock, falling back to state data"
                );
                state.get_pipeline_data().await
            }
        }
    } else {
        state.get_pipeline_data().await
    };

    pipeline_data.opentelemetry_status = get_opentelemetry_status().await;

    Ok(Json(pipeline_data))
}

#[allow(dead_code)]
async fn get_opentelemetry_status() -> Option<OpenTelemetryStatus> {
    match OtelCollector::new() {
        Ok(collector) => {
            let enabled = collector.is_running();
            let version = collector.get_version();
            let pid = if enabled {
                let pid_file = &crate::utils::workdir::TRACER_WORK_DIR.otel_pid_file;
                if pid_file.exists() {
                    std::fs::read_to_string(pid_file)
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
                endpoint: Some("otelhttp".to_string()),
            })
        }
        Err(_) => None,
    }
}
