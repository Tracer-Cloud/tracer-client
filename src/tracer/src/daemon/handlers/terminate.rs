use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::response::IntoResponse;

pub async fn terminate(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    state.cancel(); // todo: gracefully shutdown
    Ok("Terminating...")
}
