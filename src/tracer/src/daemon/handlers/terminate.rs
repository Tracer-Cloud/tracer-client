use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

pub const TERMINATE_ENDPOINT: &str = "/terminate";

pub async fn terminate(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    state.terminate_server();
    Ok(Json("Termination request sent successfully."))
}
