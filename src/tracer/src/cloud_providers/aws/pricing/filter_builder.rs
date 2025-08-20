//! Filter builders for AWS pricing queries

use aws_sdk_pricing::types::Filter as PricingFilters;

use crate::cloud_providers::aws::types::pricing::{
    EBSFilterBuilder, EC2FilterBuilder, FilterableInstanceDetails,
};

/// Build EC2 filters from instance details
pub fn build_ec2_filters(details: &FilterableInstanceDetails) -> Vec<PricingFilters> {
    EC2FilterBuilder::from_instance_details(details.clone()).to_filter()
}

/// Build EBS filters for volume type and region
pub fn build_ebs_filters(region: &str, volume_type: &str) -> Vec<PricingFilters> {
    EBSFilterBuilder {
        region: region.to_string(),
        volume_types: vec![volume_type.to_string()],
    }
    .to_filter()
}
