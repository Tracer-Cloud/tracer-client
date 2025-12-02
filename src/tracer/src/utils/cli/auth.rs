use crate::cli::handlers::auth::types::AuthType;
use crate::constants::{
    CLI_LOGIN_URL_DEV, CLI_LOGIN_URL_LOCAL, CLI_LOGIN_URL_PROD, CLI_SIGNUP_URL_DEV,
    CLI_SIGNUP_URL_LOCAL, CLI_SIGNUP_URL_PROD,
};

pub fn get_auth_url(platform: &str, auth_type: AuthType, is_development_environment: bool) -> &str {
    match auth_type {
        AuthType::Login => get_login_url(platform, is_development_environment),
        AuthType::SignUp => get_signup_url(platform, is_development_environment),
    }
}

fn get_login_url(platform: &str, is_development_environment: bool) -> &str {
    if platform.eq_ignore_ascii_case("local") {
        CLI_LOGIN_URL_LOCAL
    } else if platform.eq_ignore_ascii_case("dev") || is_development_environment {
        CLI_LOGIN_URL_DEV
    } else {
        CLI_LOGIN_URL_PROD
    }
}

fn get_signup_url(platform: &str, is_development_environment: bool) -> &str {
    if platform.eq_ignore_ascii_case("local") {
        CLI_SIGNUP_URL_LOCAL
    } else if platform.eq_ignore_ascii_case("dev") || is_development_environment {
        CLI_SIGNUP_URL_DEV
    } else {
        CLI_SIGNUP_URL_PROD
    }
}
