use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

pub const STOP_ENDPOINT: &str = "/stop";

pub async fn stop(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    Ok(Json(state.stop_client().await))
}
