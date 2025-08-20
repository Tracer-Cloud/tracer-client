//! EC2 pricing data fetching with functional retry logic

use aws_sdk_pricing as pricing;
use aws_sdk_pricing::types::Filter as PricingFilters;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;

use crate::cloud_providers::aws::types::pricing::{PricingData, ServiceCode};

/// Fetch EC2 pricing data with retry logic
pub async fn fetch_ec2_pricing_data(
    pricing_client: &pricing::Client,
    filters: Vec<PricingFilters>,
) -> Option<Vec<PricingData>> {
    retry_fetch_ec2_pricing(pricing_client, Some(filters)).await
}

/// Retry wrapper for EC2 pricing fetch
async fn retry_fetch_ec2_pricing(
    pricing_client: &pricing::Client,
    filters: Option<Vec<PricingFilters>>,
) -> Option<Vec<PricingData>> {
    let strategy = ExponentialBackoff::from_millis(500).take(3);

    Retry::spawn(strategy, {
        let filters = filters.clone();
        move || {
            let filters = filters.clone();
            async move { fetch_ec2_pricing(pricing_client, filters).await }
        }
    })
    .await
    .ok()
}

/// Core EC2 pricing fetch function
async fn fetch_ec2_pricing(
    pricing_client: &pricing::Client,
    filters: Option<Vec<PricingFilters>>,
) -> Result<Vec<PricingData>, Box<dyn std::error::Error + Send + Sync>> {
    let mut paginator = pricing_client
        .get_products()
        .service_code(ServiceCode::Ec2.as_str())
        .set_filters(filters)
        .into_paginator()
        .send();

    let mut results = Vec::new();

    while let Some(output) = paginator.next().await {
        let output = output?;
        for product in output.price_list() {
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(product) {
                if let Ok(pricing_data) = PricingData::from_json(&json_value) {
                    results.push(pricing_data);
                }
            }
        }
    }

    Ok(results)
}
