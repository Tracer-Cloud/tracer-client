use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::structs::{InfoResponse, InnerInfoResponse, Message, RunData, TagData, UploadData};
use axum::response::IntoResponse;
use axum::routing::{post, put};
use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use tracer_client::config_manager::{Config, ConfigLoader};
use tracer_client::TracerClient;
use tracer_common::debug_log::Logger;
use tracer_common::http_client::upload::upload_from_file_path;

#[derive(Clone)]
struct AppState {
    tracer_client: Arc<Mutex<TracerClient>>,
    cancellation_token: CancellationToken,
    config: Arc<RwLock<Config>>, // todo: config should only live inside Arc<TracerClient>
}

pub fn get_app(
    tracer_client: Arc<Mutex<TracerClient>>,
    cancellation_token: CancellationToken,
    config: Arc<RwLock<Config>>,
) -> Router {
    // todo: set subscriber

    let state = AppState {
        tracer_client,
        cancellation_token,
        config,
    };

    Router::new()
        .route("/log", post(log))
        .route("/terminate", post(terminate))
        .route("/start", post(start))
        .route("/end", post(end))
        .route("/alert", post(alert))
        .route("/refresh-config", post(refresh_config))
        .route("/tag", post(tag))
        .route(
            "/log-short-lived-process",
            put(log_short_lived_process_command),
        )
        .route("/info", get(info))
        .route("/upload", put(upload))
        .with_state(state)
}

async fn terminate(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    state.cancellation_token.cancel(); // todo: gracefully shutdown
    Ok("Terminating...")
}

async fn log(
    State(state): State<AppState>,
    Json(message): Json<Message>,
) -> axum::response::Result<impl IntoResponse> {
    state
        .tracer_client
        .lock()
        .await
        .send_log_event(message.payload)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}

async fn alert(
    State(state): State<AppState>,
    Json(message): Json<Message>,
) -> axum::response::Result<impl IntoResponse> {
    state
        .tracer_client
        .lock()
        .await
        .send_alert_event(message.payload)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}

async fn start(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.tracer_client.lock().await;

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

async fn end(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.tracer_client.lock().await;

    guard
        .stop_run()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}

async fn refresh_config(
    State(state): State<AppState>,
) -> axum::response::Result<impl IntoResponse> {
    // todo: IO in load condig has to be async
    let config_file =
        ConfigLoader::load_config(None).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    {
        let mut guard = state.tracer_client.lock().await;
        guard
            .reload_config_file(config_file.clone())
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    state.config.write().await.clone_from(&config_file);

    Ok(StatusCode::ACCEPTED)
}

async fn tag(
    State(state): State<AppState>,
    Json(payload): Json<TagData>,
) -> axum::response::Result<impl IntoResponse> {
    let guard = state.tracer_client.lock().await;
    guard
        .send_update_tags_event(payload.names)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}

async fn log_short_lived_process_command() -> axum::response::Result<impl IntoResponse> {
    // todo: remove the endpoint

    Ok(StatusCode::CREATED)
}

async fn info(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.tracer_client.lock().await;

    let pipeline = guard.get_run_metadata().read().await.clone();

    let response_inner = InnerInfoResponse::try_from(pipeline).ok();

    let preview = guard.process_watcher.preview_targets(10).await;
    let preview_len = guard.process_watcher.targets_len().await;

    let output = InfoResponse::new(preview, preview_len, response_inner);

    Ok(Json(output))
}

async fn upload(
    State(state): State<AppState>,
    Json(payload): Json<UploadData>,
) -> axum::response::Result<impl IntoResponse> {
    let guard = state.tracer_client.lock().await;

    let logger = Logger::new();
    logger.log("app//process_upload_command", None).await;

    // todo: upload should happen as a part of `TracerClient`
    upload_from_file_path(
        guard.get_service_url(),
        guard.get_api_key(),
        payload.file_path.as_str(),
        payload.socket_path.as_deref(),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    logger.log("process_upload_command completed", None).await;
    Ok(StatusCode::ACCEPTED)
}
