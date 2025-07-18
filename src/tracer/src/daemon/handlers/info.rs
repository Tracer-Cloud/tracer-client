use crate::client::TracerClient;
use crate::daemon::state::DaemonState;
use crate::daemon::structs::{InfoResponse, InnerInfoResponse};
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
    let response_inner = InnerInfoResponse::try_from(pipeline).ok();

    let processes = client.ebpf_watcher.get_monitored_processes().await;

    let tasks = client.ebpf_watcher.get_matched_tasks().await;

    InfoResponse::new(response_inner, processes, tasks)
}
