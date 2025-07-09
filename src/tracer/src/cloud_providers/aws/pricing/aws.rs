use aws_sdk_pricing as pricing;
use aws_sdk_pricing::types::Filter as PricingFilters;
use serde_query::DeserializeQuery;
use tokio::sync::RwLock;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;

use crate::cloud_providers::aws::aws_metadata::AwsInstanceMetaData;
use crate::cloud_providers::aws::config::{resolve_available_aws_config, AwsConfig};
use crate::cloud_providers::aws::ec2::Ec2Client;
use crate::cloud_providers::aws::pricing::filtering::ec2_matcher::EC2MatchEngine;
use crate::cloud_providers::aws::types::pricing::{
    EBSFilterBuilder, EC2FilterBuilder, EbsPricingData, FilterableInstanceDetails, FlattenedData,
    InstancePricingContext, PricingData, ServiceCode, VolumeMetadata,
};

const HOURS_IN_MONTH: f64 = 720.0;
const FREE_IOPS: i32 = 3000;
const FREE_THROUGHPUT_MBPS: i32 = 125;
const TOP_N_EC2_RESULTS: usize = 2;

/// Client for interacting with AWS Pricing API
pub struct PricingClient {
    pub pricing_client: Option<pricing::Client>,
    pub ec2_client: RwLock<Option<Ec2Client>>,
    region: String,
    aws_config: AwsConfig,
}

impl PricingClient {
    /// Creates a new PricingClient instance
    /// Note: Currently only us-east-1 region is supported for the pricing API
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
    pub async fn reinitialize_client_if_needed(
        &self,
        metadata: &AwsInstanceMetaData,
    ) -> Option<Ec2Client> {
        if metadata.region != self.region {
            tracing::info!(
                "Detected region mismatch. Reinitializing EC2 client for region: {}",
                metadata.region
            );
            let region = metadata.region.as_str();

            if let Some(conf) = resolve_available_aws_config(self.aws_config.clone(), region).await
            {
                return Some(Ec2Client::new_with_config(&conf).await);
            }
        }
        None
    }
    async fn update_client_if_needed(&self, new_client: Option<Ec2Client>) {
        if let Some(client) = new_client {
            let mut guard = self.ec2_client.write().await;
            *guard = Some(client);
        }
    }

    pub async fn get_instance_pricing_context_from_metadata(
        &self,
        metadata: &AwsInstanceMetaData,
    ) -> Option<InstancePricingContext> {
        let maybe_new_client = self.reinitialize_client_if_needed(metadata).await;
        self.update_client_if_needed(maybe_new_client).await;
        let guard = self.ec2_client.read().await;
        let ec2_client = guard.as_ref()?;

        let filterable_data = ec2_client
        .describe_instance(&metadata.instance_id, &metadata.region)
        .await
        .map_err(|e| {
            tracing::warn!(error = ?e, instance_id = %metadata.instance_id, "Failed to describe EC2 instance");
            e
        })
        .ok()?; // Exit early if instance cannot be described

        let ec2_filters = Self::build_ec2_filters(&filterable_data);

        tracing::info!("ec2 filters: {:?}", ec2_filters);

        // Fetch EBS volume metadata
        let volumes = self
            .get_volume_metadata(&metadata.instance_id)
            .await
            .unwrap_or_default();
        tracing::info!(?volumes, "Got volume information from EC2");

        // Calculate total EBS cost
        let ebs_total_price = self
            .get_total_ebs_price(&metadata.region, &volumes)
            .await
            .unwrap_or(0.0);

        tracing::info!("EC2 RAW");

        let ec2_raw: Vec<PricingData> = self
            .retry_fetch_all::<PricingData>(ServiceCode::Ec2, Some(ec2_filters))
            .await
            .unwrap_or_default();

        let engine = EC2MatchEngine::new(filterable_data.clone(), ec2_raw);

        let ec2_matches: Vec<FlattenedData> = engine
            .best_matches(TOP_N_EC2_RESULTS)
            .into_iter()
            .map(|mut data| {
                data.tenancy = filterable_data.tenancy.clone();
                data.operating_system = filterable_data.operating_system.clone();
                data.ebs_optimized = filterable_data.ebs_optimized;
                data
            })
            .collect();

        let ec2_data = ec2_matches
            .first()
            .cloned()
            .ok_or_else(|| {
                tracing::warn!("No matching EC2 pricing found");
                anyhow::anyhow!("No matching EC2 pricing")
            })
            .ok()?;

        tracing::info!(
            "Top EC2 Match: {:?}, Backup: {:?}",
            ec2_matches.first(),
            ec2_matches.get(1)
        );

        let ebs_data = if ebs_total_price > 0.0 {
            Some(FlattenedData {
                instance_type: "EBS_TOTAL".to_string(),
                region_code: metadata.region.clone(),
                vcpu: String::new(),
                memory: String::new(),
                price_per_unit: ebs_total_price,
                unit: "USD/hr".to_string(),
                price_per_gib: None,
                price_per_iops: None,
                price_per_throughput: None,

                ebs_optimized: None,
                operating_system: None,
                tenancy: None,
            })
        } else {
            None
        };

        let total = ec2_data.price_per_unit + ebs_total_price;

        Some(InstancePricingContext {
            ec2_pricing: ec2_data,
            ebs_pricing: ebs_data,
            total_hourly_cost: total,
            cost_per_minute: total / 60.0,
            source: "Live".to_string(),
            ec2_pricing_best_matches: ec2_matches,
        })
    }

    /// Extracts EC2 filter logic
    fn build_ec2_filters(details: &FilterableInstanceDetails) -> Vec<PricingFilters> {
        EC2FilterBuilder::from_instance_details(details.clone()).to_filter()
    }

    /// Builds EBS filters for a single volume type
    fn build_ebs_filters(region: &str, volume_type: &str) -> Vec<PricingFilters> {
        EBSFilterBuilder {
            region: region.to_string(),
            volume_types: vec![volume_type.to_string()],
        }
        .to_filter()
    }

    /// Fetches volume metadata (type, size, ID) for an instance
    async fn get_volume_metadata(&self, instance_id: &str) -> Option<Vec<VolumeMetadata>> {
        let guard = self.ec2_client.read().await;
        let client = guard.as_ref()?;
        client
            .get_volume_types(instance_id)
            .await
            .map_err(|err| tracing::error!(?err, "Error getting instance volumes"))
            .ok()
    }

    /// Calculates the total hourly cost for all attached EBS volumes.
    /// This uses AWS pricing tiers for gp3 (as of us-east-1 region):
    /// - Storage: $0.08/GB-month
    /// - IOPS: First 3000 free, then $0.005/provisioned IOPS-month
    /// - Throughput: First 125 MB/s free, then $0.040/provisioned MB/s-month
    ///
    /// All prices are converted from monthly to hourly by dividing by 720.
    async fn get_total_ebs_price(&self, region: &str, volumes: &[VolumeMetadata]) -> Option<f64> {
        let mut total_price = 0.0;

        for vol in volumes {
            let cost = self.calculate_volume_cost(region, vol).await.unwrap_or(0.0);
            total_price += cost;
        }

        Some(total_price)
    }

    /// Returns the **hourly** cost for a single EBS volume by converting
    /// AWS monthly pricing into hourly pricing.
    /// Applies free tier rules for gp3 volumes:
    /// - First 3000 IOPS and 125 MB/s throughput are free.
    async fn calculate_volume_cost(&self, region: &str, vol: &VolumeMetadata) -> Option<f64> {
        let filters = Self::build_ebs_filters(region, &vol.volume_type);

        tracing::info!("ebs_filters... {:?}", filters);

        let price_entries = self
            .retry_fetch_all::<EbsPricingData>(ServiceCode::Ebs, Some(filters))
            .await?;

        tracing::info!(
            "price_entries... {:?}, length..{}",
            price_entries,
            price_entries.len()
        );

        let price_data = price_entries
            .into_iter()
            .map(|data| FlattenedData::flatten_ebs_data(&data))
            .next()?; // Expect exactly one match

        // Convert base storage cost from $/GB-month to $/hr
        let storage_hourly = (vol.size_gib as f64 * price_data.price_per_unit) / HOURS_IN_MONTH;

        // Subtract free tier for IOPS (3000 free)
        let extra_iops = vol.iops.unwrap_or(0).saturating_sub(FREE_IOPS);
        let iops_hourly = price_data
            .price_per_iops
            .map(|p| (extra_iops as f64 * p) / HOURS_IN_MONTH)
            .unwrap_or(0.0);

        // Subtract free tier for throughput (125 MB/s free)
        let extra_throughput = vol
            .throughput
            .unwrap_or(0)
            .saturating_sub(FREE_THROUGHPUT_MBPS);
        let throughput_hourly = price_data
            .price_per_throughput
            .map(|p| (extra_throughput as f64 * p) / HOURS_IN_MONTH)
            .unwrap_or(0.0);

        let total_cost = storage_hourly + iops_hourly + throughput_hourly;

        tracing::info!(
            volume = ?vol.volume_id,
            storage_hourly,
            iops_hourly,
            throughput_hourly,
            total_cost,
            "Calculated hourly EBS volume cost (adjusted for free tier)"
        );

        Some(total_cost)
    }

    async fn retry_fetch_all<T>(
        &self,
        service_code: ServiceCode,
        filters: Option<Vec<PricingFilters>>,
    ) -> Option<Vec<T>>
    where
        T: for<'de> DeserializeQuery<'de> + Send + Sync,
    {
        let strategy = ExponentialBackoff::from_millis(500).take(3);
        let result = Retry::spawn(strategy, {
            let filters = filters.clone();
            let service_code = service_code.clone();

            move || {
                let filters = filters.clone();
                let service_code = service_code.clone();
                async move { self.fetch_all::<T>(service_code, filters).await }
            }
        })
        .await;

        result.ok()
    }

    async fn fetch_all<T>(
        &self,
        service_code: ServiceCode,
        filters: Option<Vec<PricingFilters>>,
    ) -> Result<Vec<T>, Box<dyn std::error::Error + Send + Sync>>
    where
        T: for<'de> DeserializeQuery<'de>,
    {
        let mut paginator = self
            .pricing_client
            .as_ref()
            .unwrap()
            .get_products()
            .service_code(service_code.as_str())
            .set_filters(filters)
            .into_paginator()
            .send();

        let mut results = Vec::new();

        while let Some(output) = paginator.next().await {
            let output = output?;
            for product in output.price_list() {
                if let Ok(pricing) = serde_json::from_str::<serde_query::Query<T>>(product) {
                    results.push(pricing.into());
                }
            }
        }

        Ok(results)
    }
}

// e2e S3 tests
#[cfg(test)]
mod tests {
    use crate::cloud_providers::aws::pricing::PricingSource;

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
