use crate::daemon::state::DaemonState;
use crate::daemon::structs::OpenTelemetryStatus;
use crate::opentelemetry::collector::OtelCollector;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use tracing::error;

pub const INFO_ENDPOINT: &str = "/info";

pub async fn info(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    println!("=== INFO HANDLER: Entry ===");
    error!("=== INFO HANDLER: Entry ===");

    println!("=== INFO HANDLER: Getting tracer client ===");
    let client_arc = state.get_tracer_client().await;
    println!("=== INFO HANDLER: Got tracer client: {:?} ===", client_arc.is_some());

    println!("=== INFO HANDLER: About to lock client ===");
    error!("=== INFO HANDLER: About to lock client ===");

    let mut pipeline_data = if let Some(client) = client_arc {
        println!("=== INFO HANDLER: Client exists, trying to lock ===");
        match tokio::time::timeout(std::time::Duration::from_millis(500), client.lock()).await {
            Ok(client) => {
                println!("=== INFO HANDLER: Lock acquired ===");
                client.get_pipeline_data().await
            },
            Err(_) => {
                println!("=== INFO HANDLER: Timeout, using state data ===");
                tracing::warn!(
                    "Timeout waiting for tracer client lock, falling back to state data"
                );
                state.get_pipeline_data().await
            }
        }
    } else {
        println!("=== INFO HANDLER: No client, using state data ===");
        state.get_pipeline_data().await
    };

    println!("=== INFO HANDLER: Got pipeline data ===");
    pipeline_data.opentelemetry_status = get_opentelemetry_status().await;
    println!("=== INFO HANDLER: Got otel status ===");

    println!("=== INFO HANDLER: Returning response ===");
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
