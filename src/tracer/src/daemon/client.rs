use super::structs::{InfoResponse, RunData};
use crate::daemon::handlers::end::END_ENDPOINT;
use crate::daemon::handlers::info::INFO_ENDPOINT;
use crate::daemon::handlers::start::START_ENDPOINT;
use crate::daemon::handlers::terminate::TERMINATE_ENDPOINT;
use reqwest::Response;
pub use reqwest::Result;

pub struct DaemonClient {
    base_uri: String,
    pub client: reqwest::Client,
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

    pub async fn send_start_run_request(&self) -> Result<Option<RunData>> {
        self.client
            .post(self.get_url(START_ENDPOINT))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    pub async fn send_terminate_request(&self) -> Result<()> {
        self.client
            .post(self.get_url(TERMINATE_ENDPOINT))
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
    }

    pub async fn send_end_request(&self) -> Result<()> {
        self.client
            .post(self.get_url(END_ENDPOINT))
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
    }

    pub async fn send_info_request(&self) -> Result<InfoResponse> {
        self.send_info().await?.error_for_status()?.json().await
    }

    pub async fn send_info(&self) -> Result<Response> {
        self.client.get(self.get_url(INFO_ENDPOINT)).send().await
    }
}
