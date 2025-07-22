use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::response::IntoResponse;

pub const TERMINATE_ENDPOINT: &str = "/terminate";

pub async fn terminate(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    state.cancel();
    Ok("Terminating...")
}
