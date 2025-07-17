use crate::client::TracerClient;
use crate::daemon::state::DaemonState;
use crate::daemon::structs::{InfoResponse, InnerInfoResponse};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use tracing::{info};

pub const INFO_ENDPOINT: &str = "/info";

pub async fn info(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    println!("Handling /info request: acquiring tracer_client lock");
    info!("Handling /info request: acquiring tracer_client lock");
    let guard = state.get_tracer_client().await;
    println!("Acquired tracer_client lock, calling get_info_response");
    info!("Acquired tracer_client lock, calling get_info_response");
    let response = get_info_response(&guard).await;
    println!("get_info_response completed, returning JSON");
    info!("get_info_response completed, returning JSON");
    Ok(Json(response))
}

pub async fn get_info_response(client: &TracerClient) -> InfoResponse {
    println!("get_info_response: acquiring run_metadata read lock");
    info!("get_info_response: acquiring run_metadata read lock");
    let pipeline = client.get_run_metadata().read().await.clone();
    println!("get_info_response: acquired run_metadata, building InnerInfoResponse");
    info!("get_info_response: acquired run_metadata, building InnerInfoResponse");
    let response_inner = InnerInfoResponse::try_from(pipeline).ok();
    println!("get_info_response: getting monitored processes");
    info!("get_info_response: getting monitored processes");
    let processes = client.ebpf_watcher.get_monitored_processes().await;
    println!("get_info_response: got monitored processes, building InfoResponse");
    info!("get_info_response: got monitored processes, building InfoResponse");

    InfoResponse::new(response_inner, processes)
}
