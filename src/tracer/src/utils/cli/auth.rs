use crate::cli::handlers::auth::types::AuthType;
use crate::constants::{
    AUTH_REDIRECT_URL_DEV_SUCCESS, AUTH_REDIRECT_URL_LOCAL_SUCCESS, AUTH_REDIRECT_URL_PROD_SUCCESS,
    LOGIN_URL_DEV, LOGIN_URL_LOCAL, LOGIN_URL_PROD, SIGNUP_URL_DEV, SIGNUP_URL_LOCAL,
    SIGNUP_URL_PROD,
};

pub fn get_auth_url(platform: &str, auth_type: AuthType, is_development_environment: bool) -> &str {
    match auth_type {
        AuthType::Login => get_login_url(platform, is_development_environment),
        AuthType::SignUp => get_signup_url(platform, is_development_environment),
    }
}

fn get_login_url(platform: &str, is_development_environment: bool) -> &str {
    get_url_based_on_platform_and_environment(
        platform,
        is_development_environment,
        LOGIN_URL_LOCAL,
        LOGIN_URL_DEV,
        LOGIN_URL_PROD,
    )
    .as_str()
}

fn get_signup_url(platform: &str, is_development_environment: bool) -> &str {
    get_url_based_on_platform_and_environment(
        platform,
        is_development_environment,
        SIGNUP_URL_LOCAL,
        SIGNUP_URL_DEV,
        SIGNUP_URL_PROD,
    )
    .as_str()
}

pub fn get_auth_redirect_url(platform: &str, is_development_environment: bool) -> &str {
    get_url_based_on_platform_and_environment(
        platform,
        is_development_environment,
        AUTH_REDIRECT_URL_LOCAL_SUCCESS,
        AUTH_REDIRECT_URL_DEV_SUCCESS,
        AUTH_REDIRECT_URL_PROD_SUCCESS,
    )
    .as_str()
}

fn get_url_based_on_platform_and_environment(
    platform: &str,
    is_development_environment: bool,
    local_url: &str,
    dev_url: &str,
    prod_url: &str,
) -> String {
    match platform.to_lowercase().as_str() {
        "local" => local_url.to_string(),
        "prod" => prod_url.to_string(),
        "default" => {
            if is_development_environment {
                dev_url.to_string()
            } else {
                prod_url.to_string()
            }
        }
        _ => dev_url.to_string(),
    }
}
