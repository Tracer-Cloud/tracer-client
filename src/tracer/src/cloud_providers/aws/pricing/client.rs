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
    ec2_region: RwLock<String>, // Track the EC2 client's region separately
    aws_config: AwsConfig,
}

impl PricingClient {
    /// Creates a new PricingClient instance
    /// The ec2_client will be reinitialized to the correct region when needed
    pub async fn new(initialization_conf: AwsConfig, initial_region: &'static str) -> Self {
        // Pricing API requires us-east-1
        let pricing_config =
            resolve_available_aws_config(initialization_conf.clone(), "us-east-1").await;

        // EC2 client starts with initial_region, will be reinitialized as needed
        let ec2_config =
            resolve_available_aws_config(initialization_conf.clone(), initial_region).await;

        match (pricing_config, ec2_config) {
            (Some(ref p_conf), Some(ref e_conf)) => Self {
                pricing_client: Some(pricing::client::Client::new(p_conf)),
                ec2_client: RwLock::new(Some(Ec2Client::new_with_config(e_conf).await)),
                aws_config: initialization_conf,
                ec2_region: RwLock::new(initial_region.to_string()),
            },
            (Some(ref p_conf), None) => Self {
                pricing_client: Some(pricing::client::Client::new(p_conf)),
                ec2_client: RwLock::new(None),
                ec2_region: RwLock::new(initial_region.to_string()),
                aws_config: initialization_conf,
            },
            _ => Self {
                pricing_client: None,
                ec2_client: RwLock::new(None),
                ec2_region: RwLock::new(initial_region.to_string()),
                aws_config: initialization_conf,
            },
        }
    }

    /// Get complete pricing context for an instance using functional composition
    pub async fn get_instance_pricing_context_from_metadata(
        &self,
        metadata: &AwsInstanceMetaData,
    ) -> Option<InstancePricingContext> {
        // Check if EC2 client needs reinitialization for the correct region
        let current_region = self.ec2_region.read().await.clone();
        let client_is_none = {
            let guard = self.ec2_client.read().await;
            guard.is_none()
        };
        
        // Initialize client if it's None or if region mismatch detected
        let maybe_new_client = if client_is_none {
            // Client is None, try to initialize it for the metadata's region
            tracing::info!(
                "EC2 client is None, initializing for region: {}",
                metadata.region
            );
            if let Some(conf) = resolve_available_aws_config(self.aws_config.clone(), &metadata.region).await {
                Some(Ec2Client::new_with_config(&conf).await)
            } else {
                None
            }
        } else {
            // Client exists, check for region mismatch
            reinitialize_client_if_needed(&self.aws_config, &current_region, metadata).await
        };

        if maybe_new_client.is_some() {
            // Update the region tracking
            let mut region_guard = self.ec2_region.write().await;
            *region_guard = metadata.region.clone();
        }

        update_client_if_needed(&self.ec2_client, maybe_new_client).await;

        let guard = self.ec2_client.read().await;
        let ec2_client = guard.as_ref()?;

        build_pricing_context(self.pricing_client.as_ref()?, ec2_client, metadata).await
    }
}
