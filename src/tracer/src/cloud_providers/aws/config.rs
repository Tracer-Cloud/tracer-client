use aws_config::{BehaviorVersion, Region, SdkConfig};
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

//AWS SDK may fallback to IMDS if running inside EC2.
pub async fn get_initialized_aws_conf(
    initialization_conf: AwsConfig,
    region: impl Into<String>,
) -> Option<SdkConfig> {
    let config_loader = aws_config::defaults(BehaviorVersion::latest());
    let loader = match initialization_conf {
        AwsConfig::Profile(profile) => {
            tracing::debug!("Trying to load AWS config using profile '{}'", profile);
            config_loader.profile_name(profile)
        }
        AwsConfig::RoleArn(arn) => {
            tracing::debug!("Trying to assume role '{}'", &arn);
            let assumed_role_provider = aws_config::sts::AssumeRoleProvider::builder(&arn)
                .session_name("tracer-client-session")
                .build()
                .await;

            let assumed_credentials_provider =
                match assumed_role_provider.provide_credentials().await {
                    Ok(creds) => creds,
                    Err(err) => {
                        tracing::warn!("Failed to assume role '{}': {:?}", arn, err);
                        return None;
                    }
                };

            config_loader.credentials_provider(assumed_credentials_provider)
        }
        AwsConfig::Env => {
            tracing::debug!("Trying to load AWS config from environment (EC2/IMDS)");
            aws_config::from_env()
        }
    };

    let config = loader.region(Region::new(region.into())).load().await;
    let credentials_provider = config.credentials_provider()?;

    match credentials_provider.provide_credentials().await {
        Ok(_) => {
            tracing::debug!("Successfully retrieved AWS credentials");
            Some(config)
        }
        Err(err) => {
            tracing::warn!("Failed to get AWS credentials: {:?}", err);
            None
        }
    }
}

pub async fn resolve_available_aws_config(profile: AwsConfig, region: &str) -> Option<SdkConfig> {
    if let AwsConfig::Profile(profile_name) = &profile {
        let profile_conf = get_initialized_aws_conf(profile.clone(), region).await;
        if profile_conf.is_some() {
            tracing::info!("Resolved AWS credentials using profile '{}'", profile_name);
            return profile_conf;
        } else {
            tracing::warn!(
                "Failed to resolve credentials using profile '{}'",
                profile_name
            );
        }
    }

    let env_conf = get_initialized_aws_conf(AwsConfig::Env, region).await;
    if env_conf.is_some() {
        tracing::info!("Resolved AWS credentials using environment.");
        return env_conf;
    }

    tracing::warn!("Could not resolve AWS credentials from profile or environment.");
    None
}

pub fn get_aws_default_profile() -> String {
    match dirs_next::home_dir() {
        None => "default",
        Some(path) => {
            if std::fs::read_to_string(path.join(".aws/credentials"))
                .unwrap_or_default()
                .contains("[me]")
            {
                "me"
            } else {
                "default"
            }
        }
    }
    .to_string()
}
