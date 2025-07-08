use aws_sdk_pricing::types::{Filter as PricingFilters, FilterType as PricingFilterType};

use std::collections::HashMap;

#[derive(Debug, serde_query::DeserializeQuery)]
pub struct PricingData {
    #[query(".product.attributes.instanceType")]
    pub instance_type: String,

    #[query(".product.attributes.regionCode")]
    pub region_code: String,

    #[query(".product.attributes.vcpu")]
    pub vcpu: String,

    #[query(".product.attributes.memory")]
    pub memory: String,

    #[query(".terms.OnDemand")]
    pub on_demand: HashMap<String, serde_json::Value>,
}

#[derive(Debug, serde_query::DeserializeQuery)]
pub struct EbsPricingData {
    #[query(".product.attributes.regionCode")]
    pub region_code: String,

    #[query(".product.attributes.volumeApiName")]
    pub instance_type: String, // using same field for compatibility

    #[query(".terms.OnDemand")]
    pub on_demand: HashMap<String, serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
pub struct OnDemandTerm {
    #[serde(rename = "priceDimensions", flatten)]
    pub price_dimensions: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub struct EC2FilterBuilder {
    pub instance_type: String,
    pub region: String,
    pub tenancy: Option<String>,
    pub vcpu: Option<String>,
    pub operating_system: Option<String>,
    pub ebs_optimized: Option<bool>,
    pub capacity_status: Option<String>,
}

impl EC2FilterBuilder {
    pub fn from_instance_details(details: FilterableInstanceDetails) -> Self {
        Self {
            instance_type: details.instance_type,
            region: details.region,
            tenancy: Some("Shared".to_string()), // Always override to Shared
            vcpu: details.vcpu,
            operating_system: match details.operating_system.as_deref() {
                Some(s) if s.contains("Linux") => Some("Linux".to_string()),
                Some(s) if s.contains("Windows") => Some("Windows".to_string()),
                Some(s) if s.contains("RHEL") => Some("RHEL".to_string()),
                Some(s) if s.contains("Ubuntu") => Some("Ubuntu Pro".to_string()),
                _ => None,
            },
            ebs_optimized: match details.ebs_optimized {
                Some(true) => Some(true),
                _ => None, // Only include if true
            },

            capacity_status: details.capacity_status, // <-- New field included
        }
    }

    pub fn to_filter(&self) -> Vec<PricingFilters> {
        let mut filters = vec![
            PricingFilters::builder()
                .field("instanceType".to_string())
                .value(self.instance_type.clone())
                .r#type(PricingFilterType::TermMatch)
                .build()
                .expect("failed to build instanceType filter"),
            PricingFilters::builder()
                .field("regionCode".to_string())
                .value(self.region.clone())
                .r#type(PricingFilterType::TermMatch)
                .build()
                .expect("failed to build regionCode filter"),
        ];

        if let Some(ref tenancy) = self.tenancy {
            filters.push(
                PricingFilters::builder()
                    .field("tenancy".to_string())
                    .value(tenancy.clone())
                    .r#type(PricingFilterType::TermMatch)
                    .build()
                    .expect("failed to build tenancy filter"),
            );
        }

        if let Some(ref vcpu) = self.vcpu {
            filters.push(
                PricingFilters::builder()
                    .field("vcpu".to_string())
                    .value(vcpu.clone())
                    .r#type(PricingFilterType::TermMatch)
                    .build()
                    .expect("failed to build vcpu filter"),
            );
        }

        if let Some(ref os) = self.operating_system {
            filters.push(
                PricingFilters::builder()
                    .field("operatingSystem".to_string())
                    .value(os.clone())
                    .r#type(PricingFilterType::TermMatch)
                    .build()
                    .expect("failed to build operatingSystem filter"),
            );
        }

        if let Some(cap_status) = &self.capacity_status {
            filters.push(
                PricingFilters::builder()
                    .field("capacitystatus".to_string())
                    .value(cap_status.clone())
                    .r#type(PricingFilterType::TermMatch)
                    .build()
                    .expect("failed to build capacitystatus filter"),
            );
        }

        filters
    }
}

#[derive(Clone)]
pub(crate) enum ServiceCode {
    Ec2,
    Ebs,
}

impl ServiceCode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ServiceCode::Ec2 => "AmazonEC2",
            ServiceCode::Ebs => "AmazonEC2",
        }
    }
}

#[derive(Debug)]
pub struct EBSFilterBuilder {
    pub region: String,
    pub volume_types: Vec<String>,
}

impl EBSFilterBuilder {
    pub fn to_filter(&self) -> Vec<PricingFilters> {
        let mut filters = vec![
            PricingFilters::builder()
                .field("regionCode".to_string())
                .value(self.region.clone())
                .r#type(PricingFilterType::TermMatch)
                .build()
                .expect("failed to build region filter"),
            PricingFilters::builder()
                .field("productFamily".to_string())
                .value("Storage".to_string())
                .r#type(PricingFilterType::TermMatch)
                .build()
                .expect("failed to build productFamily filter"),
        ];

        for volume_type in &self.volume_types {
            filters.push(
                PricingFilters::builder()
                    .field("volumeApiName".to_string())
                    .value(volume_type)
                    .r#type(PricingFilterType::TermMatch)
                    .build()
                    .expect("failed to build volumeApiName filter"),
            );
        }

        filters
    }
}

/// Metadata for a single attached storage volume
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VolumeMetadata {
    pub volume_id: String,
    pub volume_type: String,
    pub size_gib: i32,
    pub iops: Option<i32>,
    pub throughput: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct FilterableInstanceDetails {
    pub instance_type: String,
    pub region: String,
    pub availability_zone: String,
    pub operating_system: Option<String>, // e.g., Linux
    pub tenancy: Option<String>,          // e.g., default/shared
    pub vcpu: Option<String>,             // e.g., 8
    pub ebs_optimized: Option<bool>,      // true only if "Yes"
    pub capacity_status: Option<String>,
}
