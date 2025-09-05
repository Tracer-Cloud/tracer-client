use crate::constants::{CLI_LOGIN_URL_DEV, JWT_TOKEN_FILE_PATH, JWT_TOKEN_FOLDER_PATH};
use crate::utils::browser::browser_utils;
use crate::utils::jwt_utils::jwt::is_jwt_valid;
use axum::{routing::get, Router, extract::Query};
use std::{fs, sync::{Arc, Mutex}};
use std::collections::HashMap;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use crate::daemon::server::daemon_server::create_listener;
use std::time::SystemTime;
use axum::http::Method;
use tower_http::cors::{Any, CorsLayer};

/// open a browser window when the user types 'tracer login' to login and get the token
/// It also waits for 2 minutes max for the token to be available in a specific folder
use std::time::Duration;
use tokio::time::timeout;

pub async fn login() -> Result<String, Box<dyn std::error::Error>> {
    let login_url = CLI_LOGIN_URL_DEV;


    let now_system_date = SystemTime::now();
    // 1. open the browser window to login
    browser_utils::open_url(login_url);

    // 2. start login server with cancellation support
    let cancel_token = CancellationToken::new();

    // we should put some kind of check of the port
    let server_future = start_login_server("127.0.0.1:8085".to_string(), cancel_token.clone());

    // run server in background
    tokio::spawn(server_future);

    // 3. wait up to 2 minutes for the token file to appear
    let token = match timeout(Duration::from_secs(120), wait_for_token(now_system_date)).await {
        Ok(token_opt) => token_opt,
        Err(_) => {
            // timeout elapsed, shutdown server and return error
            cancel_token.cancel();
            return Err("Login timed out waiting for token".into());
        }
    };

    if token.is_none() {
        // ensure server shuts down
        cancel_token.cancel();
        return Err("No token found".into());
    }

    let token_value = token.unwrap();
    if !is_jwt_valid(&token_value).await.0 {
        cancel_token.cancel();
        return Err("Invalid token".into());
    }

    // cancel the server now that we have the token
    cancel_token.cancel();

    // 5. return success
    Ok("Login successful! Token stored.".to_string())

}


pub async fn start_login_server(server_url: String, cancel_token: CancellationToken) -> anyhow::Result<()> {
    println!("[DEBUG] Starting login server...");

    let listener = create_listener(server_url.clone()).await;
    println!("[DEBUG] Listener created");

    // clone token for shutdown task
    let shutdown_token = cancel_token.clone();

    let tx = Arc::new(Mutex::new(Some(cancel_token.clone())));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route(
            "/callback",
            get({
                let tx = tx.clone();
                move |Query(params): Query<HashMap<String, String>>| {
                    let tx = tx.clone();
                    async move {
                        if let Some(token) = params.get("token") {
                            let _ = fs::create_dir_all(JWT_TOKEN_FOLDER_PATH);
                            let _ = fs::write(JWT_TOKEN_FILE_PATH, token);

                            if let Some(ct) = tx.lock().unwrap().take() {
                                ct.cancel();
                            }

                            "Login successful! You can close this tab."
                        } else {
                            "No token provided"
                        }
                    }
                }
            }),
        )
        .layer(cors);

    // keep a clone of the token alive inside this function
    let _keep_alive = cancel_token.clone();

    let shutdown_future = async move {
        println!("[DEBUG] Waiting for cancel...");
        shutdown_token.cancelled().await;
        println!("[DEBUG] Cancel received, shutting down");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_future)
        .await?;

    Ok(())
}

/// wait for the token to be available in a specific folder, wait for 2 minutes max
async fn wait_for_token(date: SystemTime) -> Option<String> {
    let token_file_path = JWT_TOKEN_FILE_PATH;

    loop {
        if let Ok(metadata) = std::fs::metadata(&token_file_path) {
            if let Ok(file_modified_at) = metadata.modified() {
                if file_modified_at > date {
                    if let Ok(token) = std::fs::read_to_string(token_file_path) {
                        return Some(token);
                    }
                }
            }
        }

        // poll every 200ms
        sleep(Duration::from_millis(200)).await;
    }
}
