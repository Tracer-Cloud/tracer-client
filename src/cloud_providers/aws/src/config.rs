pub use crate::pricing::PricingClient;
pub use crate::s3::S3Client;
pub use crate::secrets::SecretsClient;
use aws_config::{BehaviorVersion, SdkConfig};
use aws_credential_types::provider::ProvideCredentials;
use config::{Value, ValueKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AwsConfig {
    Profile(String),
    RoleArn(String),
    Env,
}

impl From<AwsConfig> for ValueKind {
    fn from(value: AwsConfig) -> Self {
        match value {
            AwsConfig::Profile(profile) => {
                let mut table = HashMap::new();
                table.insert(
                    "profile".to_string(),
                    Value::new(None, Self::String(profile.to_owned())),
                );
                Self::Table(table)
            }
            AwsConfig::RoleArn(role) => {
                let mut table = HashMap::new();
                table.insert(
                    "role_arn".to_string(),
                    Value::new(None, Self::String(role.to_owned())),
                );
                Self::Table(table)
            }
            AwsConfig::Env => Self::String("env".to_string()),
        }
    }
}

pub async fn get_initialized_aws_conf(
    initialization_conf: AwsConfig,
    region: &'static str,
) -> Option<SdkConfig> {
    let config_loader = aws_config::defaults(BehaviorVersion::latest());
    let config = match initialization_conf {
        AwsConfig::Profile(profile) => config_loader.profile_name(profile),
        AwsConfig::RoleArn(arn) => {
            let assumed_role_provider = aws_config::sts::AssumeRoleProvider::builder(arn)
                .session_name("tracer-client-session")
                .build()
                .await;

            let assumed_credentials_provider =
                match assumed_role_provider.provide_credentials().await {
                    Ok(creds) => creds,
                    Err(_) => return None,
                };

            config_loader.credentials_provider(assumed_credentials_provider)
        }
        AwsConfig::Env => aws_config::from_env(),
    }
    .region(region)
    .load()
    .await;

    let credentials_provider = config.credentials_provider()?;

    match credentials_provider.provide_credentials().await {
        Ok(_) => {}
        Err(_) => return None,
    };

    Some(config)
}
