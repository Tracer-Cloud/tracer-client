// src/cli.rs
use anyhow::Result;

use super::structs::{InfoResponse, LogData, Message, RunData, TagData};

pub struct DaemonClient {
    base_uri: String,
    pub client: reqwest::Client,
}

impl DaemonClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_uri: base_url,
            client: reqwest::Client::new(), // todo: timeout, max payload?
        }
    }

    pub fn get_url(&self, path: &str) -> String {
        format!("{}{}", self.base_uri, path)
    }

    pub async fn send_log_request(&self, payload: Message) -> Result<()> {
        self.client
            .post(self.get_url("/log"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_alert_request(&self, payload: Message) -> Result<()> {
        self.client
            .post(self.get_url("/alert"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_start_run_request(&self) -> Result<Option<RunData>> {
        let data: Option<RunData> = self
            .client
            .post(self.get_url("/start"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(data)
    }

    pub async fn send_terminate_request(&self) -> Result<()> {
        self.client
            .post(self.get_url("/terminate"))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_end_request(&self) -> Result<()> {
        self.client
            .post(self.get_url("/end"))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_info_request(&self) -> Result<InfoResponse> {
        let data: InfoResponse = self
            .client
            .get(self.get_url("/info"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(data)
    }

    pub async fn send_refresh_config_request(&self) -> Result<()> {
        self.client
            .post(self.get_url("/refresh-config"))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_update_tags_request(&self, payload: TagData) -> Result<()> {
        self.client
            .post(self.get_url("/tag"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send_log_short_lived_process_request(&self, payload: LogData) -> Result<()> {
        self.client
            .put(self.get_url("/log-short-lived-process"))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
