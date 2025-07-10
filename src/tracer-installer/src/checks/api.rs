use reqwest::Client;

use super::InstallCheck;

pub struct APICheck {
    client: Client,
    endpoint: String,
}

impl APICheck {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            endpoint: String::from("https://sandbox.tracer.cloud/api/logs-forward/prod"),
        }
    }
}

#[async_trait::async_trait]
impl InstallCheck for APICheck {
    async fn check(&self) -> bool {
        self.client
            .get(&self.endpoint)
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
    fn name(&self) -> &'static str {
        "API Connectivity"
    }
    fn error_message(&self) -> String {
        "Not Successful".into()
    }

    fn success_message(&self) -> String {
        "Successful".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_check_returns_true() {
        let check = APICheck::new();
        let result = check.check().await;

        // Should return either true or false
        assert_eq!(result, true);
    }
}