use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::client::TracerClient;
use crate::config::Config;
use crate::daemon::structs::{InfoResponse, InnerInfoResponse, Message, RunData, TagData};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{extract::State, http::StatusCode, routing::get, Json, Router};

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
        .route("/info", get(info))
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
    let config_file = Config::default();

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

async fn info(State(state): State<AppState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.tracer_client.lock().await;

    let pipeline = guard.get_run_metadata().read().await.clone();

    let response_inner = InnerInfoResponse::try_from(pipeline).ok();

    let preview = guard.ebpf_watcher.get_n_monitored_processes(10).await;
    let number_of_monitored_processes =
        guard.ebpf_watcher.get_number_of_monitored_processes().await;

    let output = InfoResponse::new(preview, number_of_monitored_processes, response_inner);

    Ok(Json(output))
}
