use crate::client::TracerClient;
use crate::daemon::state::DaemonState;
use crate::daemon::structs::{InfoResponse, InnerInfoResponse};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use tracing::{info, debug, error};

pub const INFO_ENDPOINT: &str = "/info";

pub async fn info(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    info!("Handling /info request: acquiring tracer_client lock");
    let guard = state.get_tracer_client().await;
    info!("Acquired tracer_client lock, calling get_info_response");
    let response = get_info_response(&guard).await;
    info!("get_info_response completed, returning JSON");
    Ok(Json(response))
}

pub async fn get_info_response(client: &TracerClient) -> InfoResponse {
    info!("get_info_response: acquiring run_metadata read lock");
    let pipeline = client.get_run_metadata().read().await.clone();
    info!("get_info_response: acquired run_metadata, building InnerInfoResponse");
    let response_inner = InnerInfoResponse::try_from(pipeline).ok();

    info!("get_info_response: getting monitored processes");
    let processes = client.ebpf_watcher.get_monitored_processes().await;
    info!("get_info_response: got monitored processes, building InfoResponse");

    InfoResponse::new(response_inner, processes)
}
