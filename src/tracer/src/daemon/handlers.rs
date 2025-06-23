use crate::config::Config;
use crate::daemon::state::DaemonState;
use crate::daemon::structs::{InfoResponse, InnerInfoResponse, Message, RunData, TagData};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

pub(super) async fn terminate(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    state.cancel(); // todo: gracefully shutdown
    Ok("Terminating...")
}

pub(super) async fn log(
    State(state): State<DaemonState>,
    Json(message): Json<Message>,
) -> axum::response::Result<impl IntoResponse> {
    state
        .get_tracer_client()
        .await
        .send_log_event(message.payload)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}

pub(super) async fn alert(
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

pub(super) async fn start(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;

    guard
        .start_new_run(None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let metadata = guard.get_run_metadata();

    let pipeline = metadata.read().await;

    let run_data = pipeline.run.as_ref().map(|run| RunData {
        pipeline_name: pipeline.pipeline_name.clone(),
        run_name: run.name.clone(),
        run_id: run.id.clone(),
    });

    Ok(Json(run_data))
}

pub(super) async fn end(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;

    guard
        .stop_run()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}

pub(super) async fn refresh_config(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    // todo: IO in load config has to be pub(super) async
    let config_file = Config::default();

    {
        let mut guard = state.get_tracer_client().await;
        guard
            .reload_config_file(config_file.clone())
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    state.get_tracer_client().await.set_config(config_file);

    Ok(StatusCode::ACCEPTED)
}

pub(super) async fn tag(
    State(state): State<DaemonState>,
    Json(payload): Json<TagData>,
) -> axum::response::Result<impl IntoResponse> {
    let guard= state.get_tracer_client().await;
    guard
        .send_update_tags_event(payload.names)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}

pub(super) async fn info(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;
    let pipeline = guard.get_run_metadata().read().await.clone();

    let response_inner = InnerInfoResponse::try_from(pipeline).ok();

    let preview = guard.ebpf_watcher.get_n_monitored_processes(10).await;
    let number_of_monitored_processes =
        guard.ebpf_watcher.get_number_of_monitored_processes().await;

    let output = InfoResponse::new(preview, number_of_monitored_processes, response_inner);

    Ok(Json(output))
}
