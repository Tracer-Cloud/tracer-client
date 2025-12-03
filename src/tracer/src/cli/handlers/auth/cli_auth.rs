use crate::constants::{JWT_TOKEN_FILE_PATH, JWT_TOKEN_FOLDER_PATH};
use crate::daemon::server::daemon_server::create_listener;
use crate::utils::browser::browser_utils;
use crate::utils::jwt_utils::jwt::is_jwt_valid;
use axum::http::Method;
use axum::{extract::Query, routing::get, Router};
use std::collections::HashMap;
use std::time::SystemTime;
use std::{
    fs,
    sync::{Arc, Mutex},
};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};

use crate::cli::handlers::auth::types::AuthType;
use crate::utils::cli::auth::{get_auth_redirect_url, get_auth_url};
use crate::utils::env::is_development_environment;
use crate::utils::jwt_utils::claims::Claims;
use axum::response::Redirect;
use colored::Colorize;
use std::time::Duration;
use tokio::time::timeout;
use tracing::log::debug;

/// open a browser window when the user types 'tracer login' to auth and get the token
/// It also waits for 5-minute max for the token to be available in a specific folder
pub async fn auth(
    platform: &str,
    auth_type: AuthType,
) -> Result<String, Box<dyn std::error::Error>> {
    let is_development_environment =
        is_development_environment() || platform.eq_ignore_ascii_case("dev");

    // Getting the auth page url, it might redirect to the sign-in or sign-up page based on the AuthType
    let auth_page_url = get_auth_url(platform, auth_type, is_development_environment);

    println!("Opening browser window to {}", auth_page_url.cyan());
    println!(
        "If the browser doesn't open automatically, go to this URL: {}",
        auth_page_url.cyan()
    );

    // Getting the redirect url based on the platform
    let redirect_url = get_auth_redirect_url(platform, is_development_environment);

    let now_system_date = SystemTime::now();

    // open the browser window to auth
    browser_utils::open_url(&auth_page_url);

    // start a server with cancellation support
    // the cancellation token is used to shut down the server when the token is received
    let cancellation_token = CancellationToken::new();

    // TODO we should put some kind of check of the port if it's already in use
    // Google Cloud CLI use this same address for the gcloud auth auth functionality
    let server_future = start_login_server(
        "127.0.0.1:8085".to_string(),
        cancellation_token.clone(),
        redirect_url.to_string(),
        auth_page_url.to_string(),
    );

    // run server in the background
    tokio::spawn(server_future);

    // wait up to 5 minutes for the token file to appear
    let token = match timeout(Duration::from_secs(300), wait_for_token(now_system_date)).await {
        Ok(token) => token,
        Err(_) => {
            // timeout elapsed, shutdown server and return error
            cancellation_token.cancel();
            return Err("Authentication timed out waiting for token, 5 minutes passed, please try `tracer auth` again".into());
        }
    };

    if token.is_none() {
        // ensure server shuts down
        cancellation_token.cancel();
        return Err("No token found".into());
    }

    let token_value = token.unwrap();

    // the first boolean in the tuple is whether the token is valid
    // the second are the claims if the token is valid
    let jwt_validation_result: (bool, Option<Claims>) = is_jwt_valid(&token_value, platform).await;

    if !jwt_validation_result.0 {
        // this means the token is not valid
        cancellation_token.cancel();
        return Err("Invalid token".into());
    }

    if jwt_validation_result.1.is_none() {
        cancellation_token.cancel();
        return Err("Invalid token, no claims found".into());
    }

    let claims = jwt_validation_result.1.unwrap();

    let user_name = format!(" {}", &claims.get_name_from_full_name()); // getting only the first name

    // cancel the server now that we have the token
    cancellation_token.cancel();

    Ok(format!(
        "Welcome back{}! Run `tracer init` to start a new run.",
        user_name
    ))
}

pub async fn start_login_server(
    server_url: String,
    cancel_token: CancellationToken,
    redirect_url_success: String,
    redirect_url_error: String,
) -> anyhow::Result<()> {
    debug!("Starting auth server on: {}", server_url);
    let listener = create_listener(server_url.clone()).await;

    // clone token for the shutdown task
    let shutdown_token = cancel_token.clone();

    let tx = Arc::new(Mutex::new(Some(cancel_token.clone())));

    // create a CORS layer to allow all GET requests
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::OPTIONS])
        .allow_headers(Any)
        .allow_private_network(true);

    let app = Router::new()
        .route(
            "/callback",
            // Handle GET for the actual callback
            get({
                let tx = tx.clone();
                move |Query(params): Query<HashMap<String, String>>| {
                    let tx = tx.clone();
                    async move {
                        if let Some(token) = params.get("token") {
                            let _ = fs::create_dir_all(JWT_TOKEN_FOLDER_PATH);
                            let _ = fs::write(JWT_TOKEN_FILE_PATH, token);

                            if let Some(cancellation_token) = tx.lock().unwrap().take() {
                                cancellation_token.cancel();
                            }

                            return Redirect::to(&redirect_url_success);
                        }

                        Redirect::to(&redirect_url_error)
                    }
                }
            }),
        )
        .layer(cors);

    // keep a clone of the token alive inside this function
    let _keep_alive = cancel_token.clone();

    let shutdown_future = async move {
        shutdown_token.cancelled().await;
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_future)
        .await?;

    debug!("Login server stopped on: {}", server_url);

    Ok(())
}

/// wait for the token to be available in a specific folder, wait for 2 minutes max
async fn wait_for_token(date: SystemTime) -> Option<String> {
    let token_file_path = JWT_TOKEN_FILE_PATH;

    // every 1 second we check if the token file has been created
    // if it was created, we check that the modified date is after the date we started waiting
    // because the file could have been created before the date we started waiting for a previous auth
    // checking the modified date allows us to get the latest token created after the auth command has started
    loop {
        if let Ok(metadata) = fs::metadata(token_file_path) {
            if let Ok(file_modified_at) = metadata.modified() {
                if file_modified_at > date {
                    let token_result = fs::read_to_string(token_file_path);

                    return token_result.ok();
                }
            }
        }

        // poll every 1 second
        sleep(Duration::from_secs(1)).await;
    }
}
