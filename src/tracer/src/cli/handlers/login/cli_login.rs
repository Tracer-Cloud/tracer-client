use crate::constants::{CLI_LOGIN_URL_DEV, JWT_TOKEN_FILE_PATH, JWT_TOKEN_FOLDER_PATH};
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

use crate::utils::jwt_utils::claims::Claims;
/// open a browser window when the user types 'tracer login' to login and get the token
/// It also waits for 2 minutes max for the token to be available in a specific folder
use std::time::Duration;
use tokio::time::timeout;

pub async fn login() -> Result<String, Box<dyn std::error::Error>> {
    let login_url = CLI_LOGIN_URL_DEV;

    let now_system_date = SystemTime::now();

    // open the browser window to login
    browser_utils::open_url(login_url);

    // start a server with cancellation support
    // the cancellation token is used to shut down the server when the token is received
    let cancellation_token = CancellationToken::new();

    // TODO we should put some kind of check of the port if it's already in use
    // Google Cloud CLI use this for the login functionality
    let server_future =
        start_login_server("127.0.0.1:8085".to_string(), cancellation_token.clone());

    // run server in the background
    tokio::spawn(server_future);

    // wait up to 2 minutes for the token file to appear
    let token = match timeout(Duration::from_secs(120), wait_for_token(now_system_date)).await {
        Ok(token) => token,
        Err(_) => {
            // timeout elapsed, shutdown server and return error
            cancellation_token.cancel();
            return Err("Login timed out waiting for token, 2 minutes passed, please try `tracer login` again".into());
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
    let jwt_validation_result: (bool, Option<Claims>) = is_jwt_valid(&token_value).await;

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

    let user_full_name = if claims.full_name.is_none() {
        ""
    } else {
        &claims.full_name.unwrap()
    };

    // cancel the server now that we have the token
    cancellation_token.cancel();

    // 5. return success
    Ok(format!(
        "Welcome back {}! Run `tracer init` to start a new run.",
        user_full_name
    ))
}

pub async fn start_login_server(
    server_url: String,
    cancel_token: CancellationToken,
) -> anyhow::Result<()> {
    println!("Starting login server on {}", server_url);
    let listener = create_listener(server_url.clone()).await;

    // clone token for the shutdown task
    let shutdown_token = cancel_token.clone();

    println!("Login server started");
    let tx = Arc::new(Mutex::new(Some(cancel_token.clone())));

    // create a CORS layer to allow all GET requests
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::OPTIONS])
        .allow_headers(Any);

    println!("2 Starting login server on {}", server_url);
    let app = Router::new()
        .route(
            "/callback",
            get({
                let tx = tx.clone();
                println!("get on {}", server_url);
                move |Query(params): Query<HashMap<String, String>>| {
                    let tx = tx.clone();
                    async move {
                        println!("get token {}", server_url);
                        if let Some(token) = params.get("token") {
                            println!("token {}", token);
                            let _ = fs::create_dir_all(JWT_TOKEN_FOLDER_PATH);
                            let _ = fs::write(JWT_TOKEN_FILE_PATH, token);

                            if let Some(cancellation_token) = tx.lock().unwrap().take() {
                                cancellation_token.cancel();
                            }
                        }
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

    println!("Login server stopped");
    Ok(())
}

/// wait for the token to be available in a specific folder, wait for 2 minutes max
async fn wait_for_token(date: SystemTime) -> Option<String> {
    let token_file_path = JWT_TOKEN_FILE_PATH;

    // every 1 second we check if the token file has been created
    // if it was created, we check that the modified date is after the date we started waiting
    // because the file could have been created before the date we started waiting for a previous login
    // checking the modified date allows us to get the latest token created after the login command has started
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
