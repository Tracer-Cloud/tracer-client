use crate::client::TracerClient;
use crate::daemon::state::DaemonState;
use crate::daemon::structs::{InfoResponse, InnerInfoResponse};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

pub async fn info(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;
    Ok(Json(get_info_response(&guard).await))
}

async fn get_info_response(client: &TracerClient) -> InfoResponse {
    let pipeline = client.get_run_metadata().read().await.clone();

    let response_inner = InnerInfoResponse::try_from(pipeline).ok();

    let preview = client.ebpf_watcher.get_n_monitored_processes(10).await;
    let number_of_monitored_processes = client
        .ebpf_watcher
        .get_number_of_monitored_processes()
        .await;

    InfoResponse::new(preview, number_of_monitored_processes, response_inner)
}
