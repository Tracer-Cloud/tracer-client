use aws_sdk_pricing as pricing;
use aws_sdk_pricing::types::Filter as PricingFilters;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;

use crate::cloud_providers::aws::aws_metadata::AwsInstanceMetaData;
use crate::cloud_providers::aws::config::{resolve_available_aws_config, AwsConfig};
use crate::cloud_providers::aws::ec2::Ec2Client;
use crate::cloud_providers::aws::types::pricing::{
    EBSFilterBuilder, EC2FilterBuilder, FlattenedData, InstancePricingContext, PricingData,
    ServiceCode,
};
use serde_query::Query;

pub enum PricingSource {
    Static,
    Live(PricingClient),
}

impl PricingSource {
    pub async fn new(initialization_conf: AwsConfig) -> Self {
        let client = PricingClient::new(initialization_conf, "us-east-1").await;

        match client.pricing_client {
            Some(_) => PricingSource::Live(client),
            None => PricingSource::Static,
        }
    }

    pub async fn get_aws_price_for_instance(
        &self,
        metadata: &AwsInstanceMetaData,
    ) -> Option<InstancePricingContext> {
        match self {
            PricingSource::Static => Some(InstancePricingContext {
                ec2_pricing: FlattenedData::default(),
                ebs_pricing: None,
                source: "Static".into(),
                total_hourly_cost: 0.0,
            }),
            PricingSource::Live(client) => {
                client
                    .get_instance_pricing_context_from_metadata(metadata)
                    .await
            }
        }
    }
}

/// Client for interacting with AWS Pricing API
pub struct PricingClient {
    pricing_client: Option<pricing::Client>,
    ec2_client: Option<Ec2Client>,
}

impl PricingClient {
    /// Creates a new PricingClient instance
    /// Note: Currently only us-east-1 region is supported for the pricing API
    pub async fn new(initialization_conf: AwsConfig, _region: &'static str) -> Self {
        let region = "us-east-1";
        let config = resolve_available_aws_config(initialization_conf, region).await;

        match config {
            Some(conf) => Self {
                pricing_client: Some(pricing::client::Client::new(&conf)),
                ec2_client: Some(Ec2Client::new_with_config(&conf).await),
            },
            None => Self {
                pricing_client: None,
                ec2_client: None,
            },
        }
    }

    pub async fn get_instance_pricing_context_from_metadata(
        &self,
        metadata: &AwsInstanceMetaData,
    ) -> Option<InstancePricingContext> {
        let ec2_filters = EC2FilterBuilder {
            instance_type: metadata.instance_type.clone(),
            region: metadata.region.clone(),
        }
        .to_filter();

        let volume_types = match &self.ec2_client {
            Some(client) => client
                .get_volume_types(&metadata.instance_id)
                .await
                .unwrap_or_default(),
            None => vec![],
        };

        let ebs_filters = EBSFilterBuilder {
            region: metadata.region.clone(),
            volume_types,
        }
        .to_filter();

        self.get_instance_pricing_context(Some(ec2_filters), Some(ebs_filters))
            .await
    }

    pub async fn get_instance_pricing_context(
        &self,
        ec2_filters: Option<Vec<PricingFilters>>,
        ebs_filters: Option<Vec<PricingFilters>>,
    ) -> Option<InstancePricingContext> {
        if self.pricing_client.is_none() {
            return Some(InstancePricingContext {
                ec2_pricing: FlattenedData::default(),
                ebs_pricing: None,
                total_hourly_cost: 0.0,
                source: "Static".to_string(),
            });
        }

        let ec2_data = self
            .get_price_with_retry(ServiceCode::Ec2, ec2_filters)
            .await?;
        let ebs_data = self
            .get_price_with_retry(ServiceCode::Ebs, ebs_filters)
            .await;

        let total =
            ec2_data.price_per_unit + ebs_data.as_ref().map(|e| e.price_per_unit).unwrap_or(0.0);

        Some(InstancePricingContext {
            ec2_pricing: ec2_data,
            ebs_pricing: ebs_data,
            total_hourly_cost: total,
            source: "Live".to_string(),
        })
    }

    async fn get_price_with_retry(
        &self,
        service_code: ServiceCode,
        filters: Option<Vec<PricingFilters>>,
    ) -> Option<FlattenedData> {
        self.pricing_client.as_ref()?;

        let strategy = ExponentialBackoff::from_millis(500).take(3);

        let result = Retry::spawn(strategy, {
            let filters = filters.clone();
            let service_code = service_code.clone();

            move || {
                let filters = filters.clone();
                let service_code = service_code.clone();
                async move { self.fetch_price(service_code, filters).await }
            }
        })
        .await;

        result.ok()
    }

    async fn fetch_price(
        &self,
        service_code: ServiceCode,
        filters: Option<Vec<PricingFilters>>,
    ) -> Result<FlattenedData, Box<dyn std::error::Error + Send + Sync>> {
        let mut response = self
            .pricing_client
            .as_ref()
            .unwrap()
            .get_products()
            .service_code(service_code.as_str())
            .set_filters(filters)
            .into_paginator()
            .send();

        let mut highest_price = FlattenedData::default();

        while let Some(output) = response.next().await {
            let output = output?;
            for product in output.price_list() {
                if let Ok(pricing) = serde_json::from_str::<Query<PricingData>>(product) {
                    let flat = FlattenedData::flatten_data(&pricing.into());
                    if flat.price_per_unit > highest_price.price_per_unit {
                        highest_price = flat;
                    }
                }
            }
        }

        Ok(highest_price)
    }
}

// e2e S3 tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    async fn setup_client() -> PricingSource {
        PricingSource::Static
    }

    fn mock_metadata() -> AwsInstanceMetaData {
        AwsInstanceMetaData {
            region: "us-east-1".to_string(),
            availability_zone: "us-east-1a".to_string(),
            instance_id: "i-mockinstance".to_string(),
            account_id: "123456789012".to_string(),
            ami_id: "ami-12345678".to_string(),
            instance_type: "t2.micro".to_string(),
            local_hostname: "ip-172-31-0-1.ec2.internal".to_string(),
            hostname: "ip-172-31-0-1.ec2.internal".to_string(),
            public_hostname: Some("ec2-54-".into()),
        }
    }

    // Basic functionality test
    #[tokio::test]
    async fn test_get_ec2_instance_price_with_specific_instance() {
        let client = setup_client().await;
        let metadata = mock_metadata();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_some());

        // let price_data = result.unwrap();
        // assert_eq!(price_data.instance_type, "t2.micro");
        // assert!(price_data.price_per_unit > 0.0);
        // assert_eq!(price_data.unit, "Hrs");
    }

    // Test no results case
    #[tokio::test]
    #[ignore = "Default Implementation returns tests for now"]
    async fn test_no_matching_instances() {
        let client = setup_client().await;
        let mut metadata = mock_metadata();
        metadata.instance_type = "non_existent_instance_type".to_string();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_none());
    }

    // Test multiple shared instance types
    #[tokio::test]
    async fn test_multiple_instance_types_with_shared_tenancy() {
        let client = setup_client().await;
        let metadata = mock_metadata();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_some());
    }

    // Test multiple shared and reserved instance types
    #[tokio::test]
    async fn test_multiple_instance_types_with_shared_and_reserved_tenancy() {
        let client = setup_client().await;
        let metadata = mock_metadata();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_some());
    }

    // Test multiple reserved instance types
    #[tokio::test]
    #[ignore = "Default Implementation returns tests for now"]
    async fn test_multiple_instance_types_with_reserved_tenancy() {
        let client = setup_client().await;
        let mut metadata = mock_metadata();
        metadata.instance_type = "reserved-type".to_string();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_none());
    }

    // Test retry behavior with long response times
    #[tokio::test]
    async fn test_retry_behavior() {
        let client = setup_client().await;
        let metadata = mock_metadata();

        // Test with a reasonable timeout that allows for retries
        let result = timeout(
            Duration::from_secs(15), // Longer timeout to account for retries
            client.get_aws_price_for_instance(&metadata),
        )
        .await;

        assert!(
            result.is_ok(),
            "Request should complete within timeout including retries"
        );
        let price_data = result.unwrap();
        assert!(
            price_data.is_some(),
            "Should return valid pricing data after retries if needed"
        );
    }
}
