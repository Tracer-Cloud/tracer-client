use crate::utils::env;
use std::sync::LazyLock;

pub struct AuthConfig {
    pub client_id: String,
    pub auth_uri: String,
    pub token_uri: String,
    pub callback_addr: String,
    pub callback_route: String,
}

impl AuthConfig {
    pub fn callback_uri(&self) -> String {
        format!("{}{}", self.callback_addr, self.callback_route)
    }
}

pub static CLERK_CONFIG: LazyLock<AuthConfig> = LazyLock::new(|| {
    const DEFAULT_CLIENT_ID: &str = "6SqkxDcpL9PnbBHh";
    const DEFAULT_AUTH_URI: &str = "https://pleasant-sawfly-0.clerk.accounts.dev/oauth/authorize";
    const DEFAULT_TOKEN_URI: &str = "https://pleasant-sawfly-0.clerk.accounts.dev/oauth/token";
    const DEFAULT_CALLBACK_ADDR: &str = "http://127.0.0.1:8765";
    const DEFAULT_CALLBACK_ROUTE: &str = "/callback";

    let client_id =
        env::get_env_var("CLERK_CLIENT_ID").unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string());
    let auth_uri =
        env::get_env_var("CLERK_AUTH_URI").unwrap_or_else(|| DEFAULT_AUTH_URI.to_string());
    let token_uri =
        env::get_env_var("CLERK_TOKEN_URI").unwrap_or_else(|| DEFAULT_TOKEN_URI.to_string());
    let callback_addr =
        env::get_env_var("CLERK_CALLBACK_URI").unwrap_or_else(|| DEFAULT_CALLBACK_ADDR.to_string());
    let callback_route = env::get_env_var("CLERK_CALLBACK_PATH")
        .unwrap_or_else(|| DEFAULT_CALLBACK_ROUTE.to_string());

    AuthConfig {
        client_id,
        auth_uri,
        token_uri,
        callback_addr,
        callback_route,
    }
});
