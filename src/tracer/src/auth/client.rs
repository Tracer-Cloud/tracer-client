use crate::auth::AuthConfig;
use anyhow::{Error, Result};
use axum::extract::Query;
use axum::routing;
use axum::Router;
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, EndpointNotSet as NS, EndpointSet as ES,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use reqwest::{Client as HttpClient, ClientBuilder as HttpClientBuilder};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use url::Url;

type OAuthClient = BasicClient<ES, NS, NS, NS, ES>;

pub async fn auth(config: &AuthConfig) -> Result<()> {
    // create OAuth2 client
    let oauth_client = create_oauth_client(config)?;
    // create key exchange components
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    // get the URL to open in the browser
    let (auth_url, csrf_token) = get_auth_url(&oauth_client, pkce_challenge);

    // start webapp to receive callback
    // Keep state across the redirect
    let state = Arc::new(Mutex::new(Some((oauth_client, csrf_token, pkce_verifier))));
    let state_for_route = state.clone();

    // Start a tiny local HTTP server to catch the redirect -----
    let app = Router::new().route(
        "/callback",
        routing::get(move |Query(q): Query<HashMap<String, String>>| {
            let state_for_route = state_for_route.clone();
            async move {
                let code = q.get("code").cloned().ok_or("missing code")?;
                let got_state = q.get("state").cloned().ok_or("missing state")?;

                let (client, expected_csrf, pkce_verifier) = state_for_route
                    .lock()
                    .unwrap()
                    .take()
                    .ok_or("state already used")?;
                if got_state != *expected_csrf.secret() {
                    return Err::<String, String>("CSRF state mismatch".to_string());
                }

                let http_client =
                    create_http_client().map_err(|e| format!("Error creating http client: {e}"))?;

                // Exchange code for tokens
                let token = client
                    .exchange_code(AuthorizationCode::new(code))
                    .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier.secret().to_owned()))
                    .request_async(&http_client)
                    .await
                    .map_err(|e| format!("token exchange failed: {e}"))?;

                let session_jwt = token.access_token().secret().to_string();
                // In Clerk, this short-lived token is what you send to your API as Bearer auth.
                Ok::<String, String>(format!(
                    "OK, you can close this tab.\nToken (truncated): {}…",
                    &session_jwt[..std::cmp::min(16, session_jwt.len())]
                ))
            }
        }),
    );

    // Open browser to sign in
    open::that(auth_url.as_str())?;
    println!("Opening browser for sign-in… If it doesn’t open, visit:\n{auth_url}");

    // Serve callback, then exit after first request
    let addr: SocketAddr = config.callback_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn create_oauth_client(config: &AuthConfig) -> Result<OAuthClient> {
    let client = BasicClient::new(ClientId::new(config.client_id.clone()))
        .set_auth_uri(AuthUrl::new(config.auth_uri.clone())?)
        .set_token_uri(TokenUrl::new(config.token_uri.clone())?)
        .set_redirect_uri(RedirectUrl::new(config.callback_uri())?);
    Ok(client)
}

fn get_auth_url(client: &OAuthClient, pkce_challenge: PkceCodeChallenge) -> (Url, CsrfToken) {
    client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("read".to_string()))
        .add_scope(Scope::new("write".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url()
}

fn create_http_client() -> Result<HttpClient> {
    let http_client = HttpClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    Ok(http_client)
}
