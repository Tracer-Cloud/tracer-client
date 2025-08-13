use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

pub const INFO_ENDPOINT: &str = "/info";

pub async fn info(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;
    if let Some(client) = guard {
        let client = client.lock().await;
        Ok(Json(client.get_pipeline_data().await))
    } else {
        Ok(Json(state.get_pipeline_data().await))
    }
}
