use super::structs::PipelineMetadata;
use crate::daemon::handlers::get_user_id::{GetUserIdResponse, GET_USER_ID_ENDPOINT};
use crate::daemon::handlers::info::INFO_ENDPOINT;
use crate::daemon::handlers::start::START_ENDPOINT;
use crate::daemon::handlers::stop::STOP_ENDPOINT;
use crate::daemon::handlers::terminate::TERMINATE_ENDPOINT;
use crate::daemon::handlers::update_run_name::{
    UpdateRunNameRequest, UpdateRunNameResponse, UPDATE_RUN_NAME_ENDPOINT,
};
use crate::daemon::server::DaemonServer;
use crate::error_message;
use crate::utils::telemetry::presets;
use anyhow::{bail, Result};
use colored::Colorize;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};

pub struct DaemonClient {
    base_uri: String,
    client: Client,
}

impl DaemonClient {
    pub fn new(base_uri: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { base_uri, client }
    }

    async fn request<T, B>(&self, endpoint: &str, body: Option<B>) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
        B: Serialize,
    {
        if !DaemonServer::is_running() {
            error_message!("Tracer daemon is not running");
            bail!("Tracer daemon is not running");
        }

        let url = format!("{}{}", self.base_uri, endpoint);
        let builder = if body.is_some() {
            self.client.post(&url).json(&body)
        } else {
            self.client.get(&url)
        };

        let response = builder.send().await.map_err(|e| {
            let msg = format!("Network error for {}: {}", endpoint, e);
            presets::report_network_failure("daemon_client", &url, &e, &msg);
            error_message!("{}", msg);
            e
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let msg = format!("HTTP {} from {}", status, endpoint);
            presets::report_http_error("daemon_client", &url, status.as_u16(), None, None, &msg);
            error_message!("{}", msg);
            bail!("HTTP error {}", status);
        }

        response.json().await.map_err(|e| {
            let msg = format!("JSON parse error from {}: {}", endpoint, e);
            error_message!("{}", msg);
            e.into()
        })
    }

    // API methods
    pub async fn send_start_request(&self) -> Result<Option<PipelineMetadata>> {
        self.request(START_ENDPOINT, Option::<()>::None).await
    }

    pub async fn send_stop_request(&self) -> Result<bool> {
        self.request(STOP_ENDPOINT, Option::<()>::None).await
    }

    pub async fn send_terminate_request(&self) -> Result<String> {
        self.request(TERMINATE_ENDPOINT, Some(())).await
    }

    pub async fn send_info_request(&self) -> Result<PipelineMetadata> {
        self.request(INFO_ENDPOINT, Option::<()>::None).await
    }

    pub async fn send_update_run_name_request(
        &self,
        run_name: String,
    ) -> Result<UpdateRunNameResponse> {
        let req = UpdateRunNameRequest { run_name };
        self.request(UPDATE_RUN_NAME_ENDPOINT, Some(req)).await
    }

    pub async fn send_get_user_id_request(&self) -> Result<GetUserIdResponse> {
        self.request(GET_USER_ID_ENDPOINT, Option::<()>::None).await
    }

    pub async fn ping(&self) -> Result<Response> {
        if !DaemonServer::is_running() {
            bail!("Daemon not running");
        }

        let url = format!("{}{}", self.base_uri, INFO_ENDPOINT);
        self.client.get(&url).send().await.map_err(Into::into)
    }
}
