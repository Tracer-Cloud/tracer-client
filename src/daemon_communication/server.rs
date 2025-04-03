use anyhow::Result;
use serde_json::json;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::{
    io::AsyncWriteExt,
    net::UnixStream,
    sync::{Mutex, RwLock},
};
use tokio_util::sync::CancellationToken;

use crate::daemon_communication::structs::{LogData, RunData, TagData, UploadData};
use crate::tracer_client::Message;
use crate::{
    config_manager::{Config, ConfigManager},
    daemon_communication::structs::{InfoResponse, InnerInfoResponse},
    extracts::process_watcher::ShortLivedProcessLog,
    tracer_client::TracerClient,
    utils::{debug_log::Logger, upload::upload_from_file_path},
};

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};

type ProcessOutput<'a> =
    Option<Pin<Box<dyn Future<Output = Result<String, anyhow::Error>> + 'a + Send>>>;

pub fn process_start_run_command<'a>(
    tracer_client: &'a Arc<Mutex<TracerClient>>,
    stream: &'a mut UnixStream,
) -> ProcessOutput<'a> {
    async fn fun<'a>(
        tracer_client: &'a Arc<Mutex<TracerClient>>,
        stream: &'a mut UnixStream,
    ) -> Result<String, anyhow::Error> {
        tracer_client.lock().await.start_new_run(None).await?;

        let guard = tracer_client.lock().await;

        let info = guard.get_run_metadata();

        let output = if let Some(info) = info {
            json!({
                "run_name": info.name,
                "run_id": info.id,
                "pipeline_name": guard.get_pipeline_name(),
            })
        } else {
            json!({
                "run_name": "",
                "run_id": "",
                "pipeline_name": "",
            })
        };

        stream
            .write_all(serde_json::to_string(&output)?.as_bytes())
            .await?;

        stream.flush().await?;

        Ok("".to_string())
    }

    Some(Box::pin(fun(tracer_client, stream)))
}

pub fn process_log_short_lived_process_command<'a>(
    tracer_client: &'a Arc<Mutex<TracerClient>>,
    object: &serde_json::Map<String, serde_json::Value>,
) -> ProcessOutput<'a> {
    if !object.contains_key("log") {
        return None;
    };

    let log: ShortLivedProcessLog =
        serde_json::from_value(object.get("log").unwrap().clone()).unwrap();

    Some(Box::pin(async move {
        let mut tracer_client = tracer_client.lock().await;
        tracer_client.fill_logs_with_short_lived_process(log)?;
        Ok("".to_string())
    }))
}

use axum::response::IntoResponse;
use axum::routing::{post, put};

#[derive(Clone)]
struct AppState {
    tracer_client: Arc<Mutex<TracerClient>>,
    cancellation_token: CancellationToken,
    config: Arc<RwLock<Config>>,
}

pub fn get_app(
    tracer_client: Arc<Mutex<TracerClient>>,
    cancellation_token: CancellationToken,
    config: Arc<RwLock<Config>>,
) -> Router {
    // tracing_subscriber::fmt::init();

    let state = AppState {
        tracer_client: tracer_client.clone(),
        cancellation_token: cancellation_token.clone(),
        config: config.clone(),
    };

    Router::new()
        .route("/log", post(log))
        .route("/terminate", post(terminate))
        .route("/start", post(start))
        .route("/alert", post(alert))
        .route("/end", post(end))
        .route("/refresh-config", post(refresh_config))
        .route("/tag", post(tag))
        .route(
            "/log-short-lived-process",
            post(log_short_lived_process_command),
        )
        .route("/info", get(info))
        .route("/upload", put(upload))
        .with_state(state)
}

pub async fn terminate(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    state.cancellation_token.cancelled().await; // todo: gracefully shutdown
    Ok(StatusCode::ACCEPTED)
}

pub async fn log(
    State(state): State<AppState>,
    Json(payload): Json<Message>,
) -> axum::response::Result<impl IntoResponse> {
    state
        .tracer_client
        .lock()
        .await
        .send_log_event(payload)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::ACCEPTED)
}

pub async fn alert(
    State(state): State<AppState>,
    Json(payload): Json<Message>,
) -> axum::response::Result<impl IntoResponse> {
    state
        .tracer_client
        .lock()
        .await
        .send_alert_event(payload)
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
        guard.reload_config_file(&config_file);
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

    Ok(StatusCode::ACCEPTED)
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
    logger.log("server.rs//process_upload_command", None).await;

    // todo: upload should happen as a part of `TracerClient`
    upload_from_file_path(
        guard.get_service_url(),
        guard.get_api_key(),
        payload.file_path.as_str(),
        None,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    logger.log("process_upload_command completed", None).await;
    Ok(StatusCode::ACCEPTED)
}
