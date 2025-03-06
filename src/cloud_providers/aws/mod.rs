mod pricing;
mod s3;
mod secrets;

use aws_config::{BehaviorVersion, SdkConfig};
use aws_credential_types::provider::ProvideCredentials;
pub use pricing::PricingClient;
pub use s3::S3Client;
pub use secrets::SecretsClient;

#[cfg(test)]
pub use s3::tests::setup_env_vars;

use crate::types::config::AwsConfig;

async fn get_initialized_aws_conf(
    initialization_conf: AwsConfig,
    region: &'static str,
) -> SdkConfig {
    let config_loader = aws_config::defaults(BehaviorVersion::latest());
    let config = match initialization_conf {
        AwsConfig::Profile(profile) => config_loader.profile_name(profile),
        AwsConfig::RoleArn(arn) => {
            let assumed_role_provider = aws_config::sts::AssumeRoleProvider::builder(arn)
                .session_name("tracer-client-session")
                .build()
                .await;

            let assumed_credentials_provider = assumed_role_provider
                .provide_credentials()
                .await
                .expect("Failed to get assumed session role");

            config_loader.credentials_provider(assumed_credentials_provider)
        }
        AwsConfig::Env => aws_config::from_env(),
    }
    .region(region)
    .load()
    .await;

    let credentials_provider = config
        .credentials_provider()
        .expect("Failed to get credentials_provider");
    let _ = credentials_provider
        .provide_credentials()
        .await
        .expect("No Credentials Loaded");

    config
}
