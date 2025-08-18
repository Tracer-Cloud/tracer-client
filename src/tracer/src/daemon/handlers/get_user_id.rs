use crate::daemon::state::DaemonState;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

pub const GET_USER_ID_ENDPOINT: &str = "/get-user-id";

#[derive(Serialize, Deserialize)]
pub struct GetUserIdResponse {
    pub success: bool,
    pub message: String,
    pub user_id: Option<String>,
}

pub async fn get_user_id(
    State(state): State<DaemonState>,
) -> axum::response::Result<impl IntoResponse> {
    match state.get_user_id().await {
        Some(user_id) => {
            let response = GetUserIdResponse {
                success: true,
                message: "User ID retrieved successfully".to_string(),
                user_id: Some(user_id),
            };
            Ok(Json(response))
        }
        None => {
            let response = GetUserIdResponse {
                success: false,
                message: "User ID not found in daemon configuration".to_string(),
                user_id: None,
            };
            Ok(Json(response))
        }
    }
}
