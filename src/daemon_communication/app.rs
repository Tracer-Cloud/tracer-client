use anyhow::Result;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::{
    io::AsyncWriteExt,
    sync::{Mutex, RwLock},
};
use tokio_util::sync::CancellationToken;

use crate::daemon_communication::structs::{LogData, Message, RunData, TagData, UploadData};
use crate::{
    config_manager,
    config_manager::{Config, ConfigManager},
    daemon_communication::structs::{InfoResponse, InnerInfoResponse},
    tracer_client::TracerClient,
    utils::{debug_log::Logger, upload::upload_from_file_path},
};

use axum::response::IntoResponse;
use axum::routing::{post, put};
use axum::{extract::State, http::StatusCode, routing::get, Json, Router};

type ProcessOutput<'a> =
    Option<Pin<Box<dyn Future<Output = Result<String, anyhow::Error>> + 'a + Send>>>;

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
    // tracing_subscriber::fmt::init();

    let state = AppState {
        tracer_client,
        cancellation_token,
        config,
    };

    Router::new()
        .route("/log", post(log))
        .route("/terminate", post(terminate))
        .route("/terminate", get(terminate)) // todo: remove
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

pub async fn terminate(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    state.cancellation_token.cancel(); // todo: gracefully shutdown
    Ok("Terminating...")
}

pub async fn log(
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

pub async fn alert(
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

pub async fn start(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    let mut guard = state.tracer_client.lock().await;

    guard
        .start_new_run(None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let run_data = guard.get_run_metadata().map(|r| RunData {
        run_name: r.name,
        run_id: r.id,
        pipeline_name: guard.get_pipeline_name().to_string(),
    });

    Ok(Json(run_data))
}

pub async fn end(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    let mut guard = state.tracer_client.lock().await;

    guard
        .stop_run()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}

pub async fn refresh_config(
    State(state): State<AppState>,
) -> axum::response::Result<impl IntoResponse> {
    let config_file = ConfigManager::load_config();

    {
        let mut guard = state.tracer_client.lock().await;
        guard.reload_config_file(config_file.clone());
    }

    state.config.write().await.clone_from(&config_file);

    Ok(StatusCode::ACCEPTED)
}

pub async fn tag(
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

pub async fn log_short_lived_process_command(
    State(state): State<AppState>,
    Json(payload): Json<LogData>,
) -> axum::response::Result<impl IntoResponse> {
    let mut guard = state.tracer_client.lock().await;
    guard
        .fill_logs_with_short_lived_process(payload.log)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

pub async fn info(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.tracer_client.lock().await;

    let response_inner: Option<InnerInfoResponse> = guard.get_run_metadata().map(|out| out.into());

    let preview = guard.process_watcher.preview_targets();
    let preview_len = guard.process_watcher.preview_targets_count();

    let output = InfoResponse::new(preview, preview_len, response_inner);

    Ok(Json(output))
}

pub async fn upload(
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
