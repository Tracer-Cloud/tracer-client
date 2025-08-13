use super::structs::PipelineMetadata;
use crate::daemon::handlers::info::INFO_ENDPOINT;
use crate::daemon::handlers::start::START_ENDPOINT;
use crate::daemon::handlers::stop::STOP_ENDPOINT;
use crate::daemon::handlers::terminate::TERMINATE_ENDPOINT;
use crate::daemon::server::DaemonServer;
use crate::error_message;
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
        match self.unpack_response(response) {
            Some(response) => self.extract_json(response).await,
            None => {
                error_message!("Failed to send request to {}", endpoint);
                Err("Failed to send request")
            }
        }
    }
    async fn extract_json<T: serde::de::DeserializeOwned>(
        &self,
        response: Response,
    ) -> Result<T, &str> {
        match response.json::<T>().await {
            Ok(json) => Ok(json),
            Err(e) => {
                error_message!("Failed to parse JSON response: {}", e);
                Err("Failed to parse JSON response")
            }
        }
    }
    fn unpack_response(&self, response: reqwest::Result<Response>) -> Option<Response> {
        match response {
            Ok(resp) => Some(resp),
            Err(e) => {
                error_message!("Request to the daemon failed: {}", e);
                error_message!("Daemon may be unresponsive, please run `tracer cleanup-port` to resolve the issue.");
                None
            }
        }
    }
}
