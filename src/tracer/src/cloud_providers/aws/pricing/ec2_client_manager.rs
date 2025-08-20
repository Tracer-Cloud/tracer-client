//! EC2 client management with functional approach

use tokio::sync::RwLock;

use crate::cloud_providers::aws::aws_metadata::AwsInstanceMetaData;
use crate::cloud_providers::aws::config::{resolve_available_aws_config, AwsConfig};
use crate::cloud_providers::aws::ec2::Ec2Client;

/// Reinitialize EC2 client if region mismatch detected
pub async fn reinitialize_client_if_needed(
    aws_config: &AwsConfig,
    current_region: &str,
    metadata: &AwsInstanceMetaData,
) -> Option<Ec2Client> {
    if metadata.region != current_region {
        tracing::info!(
            "Detected region mismatch. Reinitializing EC2 client for region: {}",
            metadata.region
        );

        if let Some(conf) = resolve_available_aws_config(aws_config.clone(), &metadata.region).await
        {
            Some(Ec2Client::new_with_config(&conf).await)
        } else {
            None
        }
    } else {
        None
    }
}

/// Update client if new one is provided
pub async fn update_client_if_needed(
    ec2_client: &RwLock<Option<Ec2Client>>,
    new_client: Option<Ec2Client>,
) {
    if let Some(client) = new_client {
        let mut guard = ec2_client.write().await;
        *guard = Some(client);
    }
}
