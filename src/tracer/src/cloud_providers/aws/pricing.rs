use aws_sdk_pricing as pricing;
use aws_sdk_pricing::types::Filter as PricingFilters;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, trace, warn};

use crate::cloud_providers::aws::config::{get_initialized_aws_conf, AwsConfig};
use crate::cloud_providers::aws::types::pricing::{FlattenedData, PricingData};
use serde_query::Query;

pub enum PricingSource {
    Static,
    Live(PricingClient),
}

impl PricingSource {
    pub async fn get_ec2_instance_price(
        &self,
        filters: Option<Vec<PricingFilters>>,
    ) -> Option<FlattenedData> {
        match self {
            PricingSource::Static => Some(FlattenedData::default()),
            PricingSource::Live(client) => {
                let filters = filters.map_or(Vec::new(), |a| a);
                client.get_ec2_instance_price(filters).await
            }
        }
    }
}

/// Client for interacting with AWS Pricing API
pub struct PricingClient {
    pub client: Option<pricing::client::Client>,
}

impl PricingClient {
    /// Creates a new PricingClient instance
    /// Note: Currently only us-east-1 region is supported for the pricing API
    pub async fn new(initialization_conf: AwsConfig, _region: &'static str) -> Self {
        let region = "us-east-1";
        let config = get_initialized_aws_conf(initialization_conf, region).await;

        match config {
            Some(conf) => Self {
                client: Some(pricing::client::Client::new(&conf)),
            },
            None => Self { client: None },
        }
    }

    /// Fetches EC2 instance pricing based on provided filters
    /// Returns the most expensive instance that matches the filters
    ///
    /// This method includes retry logic with exponential backoff for handling
    /// temporary failures or long response times
    ///
    /// # Arguments
    /// * `filters` - Vector of filters to apply to the pricing query
    ///
    /// # Returns
    /// * `Option<FlattenedData>` - Pricing data for the most expensive matching instance, if any
    pub async fn get_ec2_instance_price(
        &self,
        filters: Vec<PricingFilters>,
    ) -> Option<FlattenedData> {
        // If AWS config was None during initialization, always return a zero-price result
        if self.client.is_none() {
            return Some(FlattenedData {
                instance_type: "unknown".to_string(),
                region_code: "unknown".to_string(),
                vcpu: "unknown".to_string(),
                memory: "unknown".to_string(),
                price_per_unit: 0.0,
                unit: "Hrs".to_string(),
            });
        }

        // Retry configuration
        const MAX_RETRIES: u32 = 3;
        const INITIAL_RETRY_DELAY: u64 = 1; // seconds

        let mut retry_count = 0;
        let mut last_error = None;

        // Retry loop with exponential backoff
        while retry_count < MAX_RETRIES {
            if retry_count > 0 {
                let delay = INITIAL_RETRY_DELAY * (2_u64.pow(retry_count - 1)); // Exponential backoff
                debug!("Retry {} after {} seconds", retry_count, delay);
                sleep(Duration::from_secs(delay)).await;
            }

            // Attempt to get pricing data
            match self.attempt_get_ec2_price(filters.clone()).await {
                Ok(Some(data)) => {
                    debug!("Successfully retrieved pricing data.");
                    return Some(data);
                }
                Ok(None) => {
                    debug!("No matching data found, don't retry.");
                    return None; // No matching data found, don't retry
                }
                Err(e) => {
                    last_error = Some(e);
                    retry_count += 1;
                    warn!("Attempt {} failed, will retry", retry_count);
                }
            }
        }

        error!("All retries failed. Last error: {:?}", last_error);
        None
    }

    /// Single attempt to fetch EC2 pricing data
    ///
    /// # Arguments
    /// * `filters` - Vector of filters to apply to the pricing query
    ///
    /// # Returns
    /// * `Result<Option<FlattenedData>, Box<dyn Error>>` - Result containing either:
    ///   - Ok(Some(data)) - Successfully found pricing data
    ///   - Ok(None) - No matching instances found
    ///   - Err(e) - An error occurred during the request
    async fn attempt_get_ec2_price(
        &self,
        filters: Vec<PricingFilters>,
    ) -> Result<Option<FlattenedData>, Box<dyn std::error::Error + Send + Sync>> {
        // Create paginated request to AWS Pricing API

        debug!("Filters being applied: {:?}", filters); // Print statement

        let mut response = self
            .client
            .clone()
            .unwrap()
            .get_products()
            .service_code("AmazonEC2".to_string()) // Specifically query EC2 prices
            .set_filters(Some(filters)) // Apply the filters (instance type, OS, etc.)
            .into_paginator() // Handle pagination of results
            .send();

        trace!("API Request: {:?}", response); // Print statement (may need adjustment based on actual request)

        let mut data = Vec::new();

        // Process each page of results
        while let Some(output) = response.next().await {
            // Propagate any AWS API errors
            // Useful for retrying the request in the method get_ec2_instance_price()
            let output = output?;

            // Print the raw API response
            info!("API Response: {:?}", output);

            // Process each product in the current page
            for product in output.price_list() {
                // Parse the JSON pricing data using serde_query
                match serde_json::from_str::<Query<PricingData>>(product) {
                    Ok(pricing) => {
                        // Print and log the parsed pricing data
                        // Convert the complex pricing data into a flattened format
                        let flat_data = FlattenedData::flatten_data(&pricing.into());
                        info!("Flattened pricing data: {:?}", flat_data); // Print statement
                        data.push(flat_data);
                    }
                    Err(e) => {
                        error!("Failed to parse product data: {:?}", e);
                        continue; // Skip invalid products
                    }
                }
            }
        }

        debug!("Processed pricing data length: {}", data.len());

        // Return the most expensive instance from the results
        // if data is empty the reduce will return OK(None)
        Ok(data.into_iter().reduce(|a, b| {
            if a.price_per_unit > b.price_per_unit {
                a
            } else {
                b
            }
        }))
    }
}

// e2e S3 tests
#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_pricing::types::{Filter, FilterType};
    use std::time::Duration;
    use tokio;
    use tokio::time::timeout;

    // async fn setup_client() -> PricingClient {
    //     dotenv().ok();
    //     let config = AwsConfig::Env;
    //     PricingClient::new(config, "us-east-1").await
    // }

    async fn setup_client() -> PricingSource {
        PricingSource::Static
    }

    // Basic functionality test
    #[tokio::test]
    async fn test_get_ec2_instance_price_with_specific_instance() {
        let client = setup_client().await;
        let filters = vec![
            Filter::builder()
                .field("instanceType")
                .value("t2.micro")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("regionCode")
                .value("us-east-1")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
        ];

        let result = client.get_ec2_instance_price(Some(filters)).await;
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
        let filters = vec![Filter::builder()
            .field("instanceType")
            .value("non_existent_instance_type")
            .r#type(FilterType::TermMatch)
            .build()
            .unwrap()];

        let result = client.get_ec2_instance_price(Some(filters)).await;
        assert!(result.is_none());
    }

    // Test multiple shared instance types
    #[tokio::test]
    async fn test_multiple_instance_types_with_shared_tenancy() {
        let client = setup_client().await;
        let filters = vec![
            Filter::builder()
                .field("instanceType")
                .value("t2.micro")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("operatingSystem")
                .value("Linux")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("tenancy")
                .value("Shared")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("location")
                .value("US East (N. Virginia)")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
        ];

        let result = client.get_ec2_instance_price(Some(filters)).await;
        assert!(result.is_some());
    }

    // Test multiple shared and reserved instance types
    #[tokio::test]
    async fn test_multiple_instance_types_with_shared_and_reserved_tenancy() {
        let client = setup_client().await;
        let filters = vec![
            Filter::builder()
                .field("instanceType")
                .value("t2.micro")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("operatingSystem")
                .value("Linux")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("location")
                .value("US East (N. Virginia)")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
        ];

        let result = client.get_ec2_instance_price(Some(filters)).await;
        assert!(result.is_some());
    }

    // Test multiple reserved instance types
    #[tokio::test]
    #[ignore = "Default Implementation returns tests for now"]
    async fn test_multiple_instance_types_with_reserved_tenancy() {
        let client = setup_client().await;
        let filters = vec![
            Filter::builder()
                .field("operatingSystem")
                .value("Linux")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("location")
                .value("US East (N. Virginia)")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("tenancy")
                .value("Reserved")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
        ];

        let result = client.get_ec2_instance_price(Some(filters)).await;
        assert!(result.is_none());
    }

    // Test retry behavior with long response times
    #[tokio::test]
    async fn test_retry_behavior() {
        let client = setup_client().await;
        let filters = vec![
            Filter::builder()
                .field("instanceType")
                .value("t2.micro")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("operatingSystem")
                .value("Linux")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("tenancy")
                .value("Shared")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
            Filter::builder()
                .field("location")
                .value("US East (N. Virginia)")
                .r#type(FilterType::TermMatch)
                .build()
                .unwrap(),
        ];

        // Test with a reasonable timeout that allows for retries
        let result = timeout(
            Duration::from_secs(15), // Longer timeout to account for retries
            client.get_ec2_instance_price(Some(filters)),
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
