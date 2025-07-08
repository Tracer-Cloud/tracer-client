use super::structs::{InfoResponse, Message, RunData, TagData};
use crate::daemon::handlers::alert::ALERT_ENDPOINT;
use crate::daemon::handlers::end::END_ENDPOINT;
use crate::daemon::handlers::info::INFO_ENDPOINT;
use crate::daemon::handlers::log::LOG_ENDPOINT;
use crate::daemon::handlers::refresh_config::REFRESH_CONFIG_ENDPOINT;
use crate::daemon::handlers::start::START_ENDPOINT;
use crate::daemon::handlers::tag::TAG_ENDPOINT;
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

    pub async fn send_log_request(&self, payload: Message) -> Result<()> {
        self.client
            .post(self.get_url(LOG_ENDPOINT))
            .json(&payload)
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
    }

    pub async fn send_alert_request(&self, payload: Message) -> Result<()> {
        self.client
            .post(self.get_url(ALERT_ENDPOINT))
            .json(&payload)
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
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

    pub async fn send_refresh_config_request(&self) -> Result<()> {
        self.client
            .post(self.get_url(REFRESH_CONFIG_ENDPOINT))
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
    }

    pub async fn send_update_tags_request(&self, payload: TagData) -> Result<()> {
        self.client
            .post(self.get_url(TAG_ENDPOINT))
            .json(&payload)
            .send()
            .await?
            .error_for_status()
            .map(|_| ())
    }
}
