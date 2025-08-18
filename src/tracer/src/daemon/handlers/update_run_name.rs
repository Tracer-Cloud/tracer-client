use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

pub const UPDATE_RUN_NAME_ENDPOINT: &str = "/update-run-name";

#[derive(Deserialize, Serialize)]
pub struct UpdateRunNameRequest {
    pub run_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct UpdateRunNameResponse {
    pub success: bool,
    pub message: String,
    pub new_run_name: Option<String>,
}

pub async fn update_run_name(
    State(state): State<DaemonState>,
    Json(request): Json<UpdateRunNameRequest>,
) -> axum::response::Result<impl IntoResponse> {
    let guard = state.get_tracer_client().await;

    if let Some(client) = guard {
        let mut client = client.lock().await;
        match client.update_run_name(request.run_name.clone()).await {
            Ok(()) => {
                let response = UpdateRunNameResponse {
                    success: true,
                    message: "Run name updated successfully".to_string(),
                    new_run_name: Some(request.run_name),
                };
                Ok(Json(response))
            }
            Err(e) => {
                let response = UpdateRunNameResponse {
                    success: false,
                    message: format!("Failed to update run name: {}", e),
                    new_run_name: None,
                };
                Ok(Json(response))
            }
        }
    } else {
        let response = UpdateRunNameResponse {
            success: false,
            message: "No active tracer client found".to_string(),
            new_run_name: None,
        };
        Ok(Json(response))
    }
}
