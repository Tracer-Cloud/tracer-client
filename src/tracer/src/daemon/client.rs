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
use colored::Colorize;
use reqwest::Response;
pub struct DaemonClient {
    base_uri: String,
    pub client: reqwest::Client,
}

enum Method {
    Get,
    Post,
}
impl DaemonClient {
    pub fn new(base_uri: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { base_uri, client }
    }

    fn get_url(&self, path: &str) -> String {
        format!("{}{}", self.base_uri, path)
    }

    pub async fn send_start_request(&self) -> Result<Option<PipelineMetadata>, &str> {
        self.send_request(START_ENDPOINT, Method::Post).await
    }

    pub async fn send_terminate_request(&self) -> Result<String, &str> {
        self.send_request(TERMINATE_ENDPOINT, Method::Post).await
    }

    pub async fn send_stop_request(&self) -> Result<bool, &str> {
        self.send_request(STOP_ENDPOINT, Method::Post).await
    }

    pub async fn send_info_request(&self) -> Result<PipelineMetadata, &str> {
        self.send_request(INFO_ENDPOINT, Method::Get).await
    }

    pub async fn send_update_run_name_request(
        &self,
        run_name: String,
    ) -> Result<UpdateRunNameResponse, &str> {
        let request = UpdateRunNameRequest { run_name };
        self.send_request_with_body(UPDATE_RUN_NAME_ENDPOINT, Method::Post, request)
            .await
    }

    pub async fn send_get_user_id_request(&self) -> Result<GetUserIdResponse, &str> {
        self.send_request(GET_USER_ID_ENDPOINT, Method::Get).await
    }

    pub async fn ping(&self) -> reqwest::Result<Response> {
        self.client.get(self.get_url(INFO_ENDPOINT)).send().await
    }

    async fn send_request<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        method: Method,
    ) -> Result<T, &str> {
        if !DaemonServer::is_running() {
            error_message!("Tracer daemon is not running");
            return Err("Tracer daemon is not running");
        }
        let response = match method {
            Method::Get => self.client.get(self.get_url(endpoint)).send().await,
            Method::Post => self.client.post(self.get_url(endpoint)).send().await,
        };
        match self.unpack_response(response, endpoint) {
            Some(response) => self.extract_json(response, endpoint).await,
            None => {
                error_message!("Failed to send request to {}", endpoint);
                Err("Failed to send request")
            }
        }
    }

    async fn send_request_with_body<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        endpoint: &str,
        method: Method,
        body: B,
    ) -> Result<T, &str> {
        if !DaemonServer::is_running() {
            error_message!("Tracer daemon is not running");
            return Err("Tracer daemon is not running");
        }
        let response = match method {
            Method::Get => {
                self.client
                    .get(self.get_url(endpoint))
                    .json(&body)
                    .send()
                    .await
            }
            Method::Post => {
                self.client
                    .post(self.get_url(endpoint))
                    .json(&body)
                    .send()
                    .await
            }
        };
        match self.unpack_response(response, endpoint) {
            Some(response) => self.extract_json(response, endpoint).await,
            None => {
                error_message!("Failed to send request to {}", endpoint);
                Err("Failed to send request")
            }
        }
    }
    async fn extract_json<T: serde::de::DeserializeOwned>(
        &self,
        response: Response,
        endpoint: &str,
    ) -> Result<T, &str> {
        let status = response.status();
        match response.json::<T>().await {
            Ok(json) => Ok(json),
            Err(e) => {
                let error_msg = format!("Failed to parse JSON response: {}", e);

                // Report JSON parsing failures to Sentry with full context
                presets::report_json_parse_failure(
                    "daemon_client",
                    endpoint,
                    status.as_u16(),
                    &e,
                    &error_msg,
                );

                error_message!("{}", error_msg);
                Err("Failed to parse JSON response")
            }
        }
    }
    fn unpack_response(
        &self,
        response: reqwest::Result<Response>,
        endpoint: &str,
    ) -> Option<Response> {
        match response {
            Ok(resp) => {
                // Check if response status is not 2XX
                if !resp.status().is_success() {
                    let status = resp.status();
                    let error_msg = format!("Daemon request failed with status: {}", status);

                    // Report non-2XX responses to Sentry with full context
                    presets::report_http_error(
                        "daemon_client",
                        endpoint,
                        status.as_u16(),
                        status.canonical_reason(),
                        None, // No response body available here
                        &error_msg,
                    );

                    error_message!("{}", error_msg);
                }
                Some(resp)
            }
            Err(e) => {
                let error_msg = format!("Request to the daemon failed: {}", e);

                // Report network failures to Sentry with full context
                presets::report_network_failure("daemon_client", endpoint, &e, &error_msg);

                error_message!("{}", error_msg);
                error_message!("Daemon may be unresponsive, please run `tracer cleanup-port` to resolve the issue.");
                None
            }
        }
    }
}
