//! AWS Pricing API client with functional composition

use aws_sdk_pricing as pricing;
use tokio::sync::RwLock;

use crate::cloud_providers::aws::aws_metadata::AwsInstanceMetaData;
use crate::cloud_providers::aws::config::{resolve_available_aws_config, AwsConfig};
use crate::cloud_providers::aws::ec2::Ec2Client;
use crate::cloud_providers::aws::types::pricing::InstancePricingContext;

use super::context_builder::build_pricing_context;
use super::ec2_client_manager::{reinitialize_client_if_needed, update_client_if_needed};

/// Functional AWS Pricing API client
pub struct PricingClient {
    pub pricing_client: Option<pricing::Client>,
    pub ec2_client: RwLock<Option<Ec2Client>>,
    region: String,
    aws_config: AwsConfig,
}

impl PricingClient {
    /// Creates a new PricingClient instance
    pub async fn new(initialization_conf: AwsConfig, region: &'static str) -> Self {
        let config = resolve_available_aws_config(initialization_conf.clone(), region).await;

        match config {
            Some(ref conf) => Self {
                pricing_client: Some(pricing::client::Client::new(conf)),
                ec2_client: RwLock::new(Some(Ec2Client::new_with_config(conf).await)),
                aws_config: initialization_conf,
                region: region.to_string(),
            },
            None => Self {
                pricing_client: None,
                ec2_client: RwLock::new(None),
                region: region.to_string(),
                aws_config: initialization_conf,
            },
        }
    }

    /// Get complete pricing context for an instance using functional composition
    pub async fn get_instance_pricing_context_from_metadata(
        &self,
        metadata: &AwsInstanceMetaData,
    ) -> Option<InstancePricingContext> {
        // Functional pipeline: reinitialize -> update -> build context
        let maybe_new_client =
            reinitialize_client_if_needed(&self.aws_config, &self.region, metadata).await;
        update_client_if_needed(&self.ec2_client, maybe_new_client).await;

        let guard = self.ec2_client.read().await;
        let ec2_client = guard.as_ref()?;

        build_pricing_context(self.pricing_client.as_ref()?, ec2_client, metadata).await
    }
}
