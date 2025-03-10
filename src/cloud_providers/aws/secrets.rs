use aws_sdk_secretsmanager::Client;

use super::get_initialized_aws_conf;

pub struct SecretsClient {
    pub client: Client,
}

impl SecretsClient {
    pub async fn new(initialization_conf: crate::types::config::AwsConfig) -> Self {
        let region = "us-east-1";
        let config = get_initialized_aws_conf(initialization_conf, region).await;

        Self {
            client: Client::new(&config),
        }
    }

    pub async fn get_secrets<T>(&self, secret_arn: &str) -> Result<T, anyhow::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        // Retrieve the secret
        self.client
            .get_secret_value()
            .secret_id(secret_arn)
            .send()
            .await?
            .secret_string()
            .ok_or_else(|| anyhow::anyhow!("No secrets found")) // Convert None to an error
            .and_then(|s| serde_json::from_str(s).map_err(Into::into)) // Deserialize and map errors
    }
}
