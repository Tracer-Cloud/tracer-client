use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
pub const START_ENDPOINT: &str = "/start";
pub async fn start(
    State(mut state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    if let Some(client) = state.start_tracer_client().await {
        let client = client.lock().await;
        return Ok(Json(Some(client.get_pipeline_data().await)));
    }
    Ok(Json(None))
}
