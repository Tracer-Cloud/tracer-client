use crate::client::TracerClient;
use crate::daemon::state::DaemonState;
use crate::daemon::structs::TagData;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

pub const TAG_ENDPOINT: &str = "/tag";

pub async fn tag(
    State(state): State<DaemonState>,
    Json(payload): Json<TagData>,
) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;
    send_update(&guard, payload).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn send_update(client: &TracerClient, payload: TagData) -> Result<(), StatusCode> {
    client
        .send_update_tags_event(payload.names)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
