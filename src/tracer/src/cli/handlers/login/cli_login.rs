use crate::constants::{JWT_TOKEN_FILE_NAME, JWT_TOKEN_FOLDER_PATH};
use crate::utils::browser::browser_utils;
use crate::utils::jwt_utils::jwt::is_jwt_valid;
use core::time::Duration;
use std::thread::sleep;
use std::time::SystemTime;

/// open a browser window when the user types 'tracer login' to login and get the token
/// It also waits for 2 minutes max for the token to be available in a specific folder
pub async fn login() -> Result<String, Box<dyn std::error::Error>> {
    let login_url = "http://localhost:3000/sign-in?cli=true".to_string();

    // open the browser window to login
    browser_utils::open_url(&login_url);

    println!("Waiting for the token to be available...");
    // check if the token is available (so we check for the token.txt file available)
    let now_date = SystemTime::now();
    let token = wait_for_token(now_date);

    if token.is_some() {
        println!("Found a login token, validating it...");
        let is_token_valid = is_jwt_valid(token.unwrap().as_str()).await;

        if is_token_valid.0 {
            Ok("Logged in successfully!".to_string())
        } else {
            Err("Error logging in, please try again".into())
        }
    } else {
        Err("Error logging in, please try again".into())
    }
}

/// wait for the token to be available in a specific folder, wait for 2 minutes max
fn wait_for_token(date: SystemTime) -> Option<String> {

    sleep(Duration::from_secs(5));

    let token_file_path = JWT_TOKEN_FOLDER_PATH.to_string() + "/" + JWT_TOKEN_FILE_NAME;

    let token: String;

    for _ in 0..60 {
        // check if the folder path exists
        let file_metadata = std::fs::metadata(&token_file_path);
        if file_metadata.is_err() {
            continue;
        } else {
            let file_modified_at = match file_metadata.unwrap().modified() {
                Ok(modified_at) => modified_at,
                Err(_) => continue,
            };

            if file_modified_at > date {
                token = std::fs::read_to_string(token_file_path).unwrap();
                return Some(token);
            }
        }

        // check if the token file is available
        sleep(Duration::from_secs(2));
    }

    None
}
