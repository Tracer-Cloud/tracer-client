use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

pub const STOP_ENDPOINT: &str = "/stop";

pub async fn stop(
    State(mut state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    Ok({
        state.stop_client().await;
        Json(())
    })
}
