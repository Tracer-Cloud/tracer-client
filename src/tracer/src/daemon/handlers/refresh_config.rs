use crate::client::TracerClient;
use crate::config::Config;
use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

pub async fn refresh_config(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    // todo: IO in load config has to be pub(super) async
    let mut guard = state.get_tracer_client().await;
    reload_config(&mut guard, Config::default()).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn reload_config(client: &mut TracerClient, config: Config) -> Result<(), StatusCode> {
    client
        .reload_config_file(config)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
