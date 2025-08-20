//! EBS pricing calculations with functional composition

use aws_sdk_pricing as pricing;
use aws_sdk_pricing::types::Filter as PricingFilters;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;

use crate::cloud_providers::aws::ec2::Ec2Client;
use crate::cloud_providers::aws::types::pricing::{
    EbsPricingData, FlattenedData, ServiceCode, VolumeMetadata,
};

use super::filter_builder::build_ebs_filters;

const HOURS_IN_MONTH: f64 = 720.0;
const FREE_IOPS: i32 = 3000;
const FREE_THROUGHPUT_MBPS: i32 = 125;

/// Calculate total EBS cost for an instance using functional composition
pub async fn calculate_total_ebs_cost(
    pricing_client: &pricing::Client,
    ec2_client: &Ec2Client,
    region: &str,
    instance_id: &str,
) -> f64 {
    let volumes = get_volume_metadata(ec2_client, instance_id)
        .await
        .unwrap_or_default();

    let mut total_cost = 0.0;
    for vol in volumes {
        let cost = calculate_volume_cost(pricing_client, region, &vol)
            .await
            .unwrap_or(0.0);
        total_cost += cost;
    }
    total_cost
}

/// Get volume metadata for an instance
async fn get_volume_metadata(
    ec2_client: &Ec2Client,
    instance_id: &str,
) -> Option<Vec<VolumeMetadata>> {
    ec2_client
        .get_volume_types(instance_id)
        .await
        .map_err(|err| tracing::error!(?err, "Error getting instance volumes"))
        .ok()
}

/// Calculate hourly cost for a single EBS volume
async fn calculate_volume_cost(
    pricing_client: &pricing::Client,
    region: &str,
    vol: &VolumeMetadata,
) -> Option<f64> {
    let filters = build_ebs_filters(region, &vol.volume_type);
    let price_entries = retry_fetch_ebs_pricing(pricing_client, filters).await?;
    let price_data = price_entries
        .into_iter()
        .map(|data| FlattenedData::flatten_ebs_data(&data))
        .next()?;

    Some(calculate_hourly_cost(vol, &price_data))
}

/// Calculate hourly cost with free tier adjustments
fn calculate_hourly_cost(vol: &VolumeMetadata, price_data: &FlattenedData) -> f64 {
    let storage_hourly = (vol.size_gib as f64 * price_data.price_per_unit) / HOURS_IN_MONTH;

    let extra_iops = vol.iops.unwrap_or(0).saturating_sub(FREE_IOPS);
    let iops_hourly = price_data
        .price_per_iops
        .map(|p| (extra_iops as f64 * p) / HOURS_IN_MONTH)
        .unwrap_or(0.0);

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

    total_cost
}

/// Retry wrapper for EBS pricing fetch
async fn retry_fetch_ebs_pricing(
    pricing_client: &pricing::Client,
    filters: Vec<PricingFilters>,
) -> Option<Vec<EbsPricingData>> {
    let strategy = ExponentialBackoff::from_millis(500).take(3);

    Retry::spawn(strategy, {
        let filters = filters.clone();
        move || {
            let filters = filters.clone();
            async move { fetch_ebs_pricing(pricing_client, Some(filters)).await }
        }
    })
    .await
    .ok()
}

/// Core EBS pricing fetch function
async fn fetch_ebs_pricing(
    pricing_client: &pricing::Client,
    filters: Option<Vec<PricingFilters>>,
) -> Result<Vec<EbsPricingData>, Box<dyn std::error::Error + Send + Sync>> {
    let mut paginator = pricing_client
        .get_products()
        .service_code(ServiceCode::Ebs.as_str())
        .set_filters(filters)
        .into_paginator()
        .send();

    let mut results = Vec::new();

    while let Some(output) = paginator.next().await {
        let output = output?;
        for product in output.price_list() {
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(product) {
                if let Ok(pricing_data) = EbsPricingData::from_json(&json_value) {
                    results.push(pricing_data);
                }
            }
        }
    }

    Ok(results)
}
