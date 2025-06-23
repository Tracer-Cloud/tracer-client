use crate::daemon::state::DaemonState;
use crate::daemon::structs::Message;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

pub async fn alert(
    State(state): State<DaemonState>,
    Json(message): Json<Message>,
) -> axum::response::Result<impl IntoResponse> {
    state
        .get_tracer_client()
        .await
        .send_alert_event(message.payload)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}
