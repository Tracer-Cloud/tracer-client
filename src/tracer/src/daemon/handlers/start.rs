use crate::daemon::state::DaemonState;
use crate::daemon::structs::RunData;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

pub async fn start(State(state): State<DaemonState>) -> axum::response::Result<impl IntoResponse> {
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
