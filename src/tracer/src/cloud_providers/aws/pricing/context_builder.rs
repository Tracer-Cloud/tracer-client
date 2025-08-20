//! Pricing context builder using functional composition

use aws_sdk_pricing as pricing;

use crate::cloud_providers::aws::aws_metadata::AwsInstanceMetaData;
use crate::cloud_providers::aws::ec2::Ec2Client;
use crate::cloud_providers::aws::pricing::filtering::ec2_matcher::EC2MatchEngine;
use crate::cloud_providers::aws::types::pricing::{
    FlattenedData, InstancePricingContext, PricingData,
};

use super::ebs_pricing::calculate_total_ebs_cost;
use super::ec2_pricing::fetch_ec2_pricing_data;
use super::filter_builder::build_ec2_filters;

const TOP_N_EC2_RESULTS: usize = 2;

/// Build complete pricing context using functional composition
pub async fn build_pricing_context(
    pricing_client: &pricing::Client,
    ec2_client: &Ec2Client,
    metadata: &AwsInstanceMetaData,
) -> Option<InstancePricingContext> {
    // Functional pipeline: describe -> filter -> fetch -> match -> combine
    let filterable_data = describe_instance(ec2_client, metadata).await?;
    let ec2_filters = build_ec2_filters(&filterable_data);
    let ec2_raw = fetch_ec2_pricing_data(pricing_client, ec2_filters).await?;
    let ec2_matches = match_ec2_instances(filterable_data.clone(), ec2_raw)?;
    let ebs_cost = calculate_total_ebs_cost(
        pricing_client,
        ec2_client,
        &metadata.region,
        &metadata.instance_id,
    )
    .await;

    combine_pricing_data(metadata, ec2_matches, ebs_cost)
}

/// Describe EC2 instance with error handling
async fn describe_instance(
    ec2_client: &Ec2Client,
    metadata: &AwsInstanceMetaData,
) -> Option<crate::cloud_providers::aws::types::pricing::FilterableInstanceDetails> {
    ec2_client
        .describe_instance(&metadata.instance_id, &metadata.region)
        .await
        .map_err(|e| {
            tracing::warn!(
                error = ?e,
                instance_id = %metadata.instance_id,
                "Failed to describe EC2 instance"
            );
            e
        })
        .ok()
}

/// Match EC2 instances using the matching engine
fn match_ec2_instances(
    filterable_data: crate::cloud_providers::aws::types::pricing::FilterableInstanceDetails,
    ec2_raw: Vec<PricingData>,
) -> Option<Vec<FlattenedData>> {
    let engine = EC2MatchEngine::new(filterable_data.clone(), ec2_raw);

    let matches: Vec<FlattenedData> = engine
        .best_matches(TOP_N_EC2_RESULTS)
        .into_iter()
        .map(|mut data| {
            data.tenancy = filterable_data.tenancy.clone();
            data.operating_system = filterable_data.operating_system.clone();
            data.ebs_optimized = filterable_data.ebs_optimized;
            data
        })
        .collect();

    if matches.is_empty() {
        tracing::warn!("No matching EC2 pricing found");
        None
    } else {
        tracing::info!(
            "Top EC2 Match: {:?}, Backup: {:?}",
            matches.first(),
            matches.get(1)
        );
        Some(matches)
    }
}

/// Combine EC2 and EBS pricing into final context
fn combine_pricing_data(
    metadata: &AwsInstanceMetaData,
    ec2_matches: Vec<FlattenedData>,
    ebs_cost: f64,
) -> Option<InstancePricingContext> {
    let ec2_data = ec2_matches.first().cloned()?;

    let ebs_data = if ebs_cost > 0.0 {
        Some(FlattenedData {
            instance_type: "EBS_TOTAL".to_string(),
            region_code: metadata.region.clone(),
            vcpu: String::new(),
            memory: String::new(),
            price_per_unit: ebs_cost,
            unit: "USD/hr".to_string(),
            price_per_gib: None,
            price_per_iops: None,
            price_per_throughput: None,
            ebs_optimized: None,
            operating_system: None,
            tenancy: None,
            match_percentage: None,
        })
    } else {
        None
    };

    let total = ec2_data.price_per_unit + ebs_cost;
    let best_match_score = ec2_matches.first().and_then(|m| m.match_percentage);

    Some(InstancePricingContext {
        ec2_pricing: ec2_data,
        ebs_pricing: ebs_data,
        total_hourly_cost: total,
        cost_per_minute: total / 60.0,
        source: "Live".to_string(),
        ec2_pricing_best_matches: ec2_matches,
        match_confidence: best_match_score,
        instance_type: metadata.instance_type.clone(),
    })
}
