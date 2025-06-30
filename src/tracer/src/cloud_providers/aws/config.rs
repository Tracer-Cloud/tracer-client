pub use crate::cloud_providers::aws::pricing::PricingClient;
pub use crate::cloud_providers::aws::s3::S3Client;
pub use crate::cloud_providers::aws::secrets::SecretsClient;
use aws_config::{BehaviorVersion, SdkConfig};
use aws_credential_types::provider::ProvideCredentials;
use config::{Value, ValueKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AwsConfig {
    Profile(String),
    RoleArn(String),
    Env,
}
impl fmt::Display for AwsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AwsConfig::Profile(profile) => write!(f, "profile:{}", profile),
            AwsConfig::RoleArn(role) => write!(f, "role_arn:{}", role),
            AwsConfig::Env => write!(f, "env"),
        }
    }
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

pub async fn resolve_available_aws_config(
    profile: AwsConfig,
    region: &'static str,
) -> Option<SdkConfig> {
    // First, try with explicit profile which is the default value for config now
    let profile_conf = get_initialized_aws_conf(profile.clone(), region).await;

    if profile_conf.is_some() {
        tracing::info!("Resolved AWS credentials using profile '{}'", profile);
        return profile_conf;
    }

    // Next, try with environment (works for EC2 / IAM role)
    let env_conf = get_initialized_aws_conf(AwsConfig::Env, region).await;

    if env_conf.is_some() {
        tracing::info!("Resolved AWS credentials using environment/EC2 role");
        return env_conf;
    }

    // Nothing worked
    tracing::warn!("Could not resolve AWS credentials from profile or environment");
    None
}
