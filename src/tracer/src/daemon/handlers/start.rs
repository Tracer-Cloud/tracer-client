use crate::client::TracerClient;
use crate::daemon::state::DaemonState;
use crate::daemon::structs::RunData;
use crate::info_message;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use colored::Colorize;

pub const START_ENDPOINT: &str = "/start";

pub async fn start(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;

    let run_data = start_run(&guard).await;
    Ok(Json(run_data))
}

async fn start_run(client: &TracerClient) -> Option<RunData> {
    client
        .start_new_run(None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        .ok();

    let metadata = client.get_run_metadata();

    let pipeline = metadata.read().await;

    let run_data = pipeline.run.as_ref().map(|run| {
        info_message!("New pipeline run started with run_id: {}", run.id);
        info_message!("Run name: {}", run.name);
        info_message!("Pipeline: {}", pipeline.pipeline_name);
        info_message!("OpenTelemetry collector will be started separately with run details");
        
        RunData {
            pipeline_name: pipeline.pipeline_name.clone(),
            run_name: run.name.clone(),
            run_id: run.id.clone(),
        }
    });

    run_data
}
