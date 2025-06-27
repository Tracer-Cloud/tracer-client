use crate::client::TracerClient;
use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

pub const END_ENDPOINT: &str = "/end";

pub async fn end(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;

    stop_run(&guard).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn stop_run(client: &TracerClient) -> Result<(), StatusCode> {
    client
        .stop_run()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
