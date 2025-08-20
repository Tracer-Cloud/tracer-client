use aws_sdk_pricing::types::{Filter as PricingFilters, FilterType as PricingFilterType};

use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug)]
pub struct PricingData {
    pub instance_type: String,
    pub region_code: String,
    pub vcpu: String,
    pub memory: String,
    pub operating_system: Option<String>,
    pub tenancy: Option<String>,
    pub capacity_status: Option<String>,
    pub on_demand: HashMap<String, serde_json::Value>,
}

impl PricingData {
    /// Extract PricingData from AWS Pricing API JSON
    pub fn from_json(value: &Value) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let product = value
            .get("product")
            .and_then(|p| p.get("attributes"))
            .ok_or("Missing product.attributes")?;

        let instance_type = extract_string_field(product, "instanceType")?;
        let region_code = extract_string_field(product, "regionCode")?;
        let vcpu = extract_string_field(product, "vcpu")?;
        let memory = extract_string_field(product, "memory")?;

        let operating_system = extract_optional_string_field(product, "operatingSystem");
        let tenancy = extract_optional_string_field(product, "tenancy");
        let capacity_status = extract_optional_string_field(product, "capacitystatus");

        let on_demand = value
            .get("terms")
            .and_then(|t| t.get("OnDemand"))
            .and_then(|od| od.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        Ok(PricingData {
            instance_type,
            region_code,
            vcpu,
            memory,
            operating_system,
            tenancy,
            capacity_status,
            on_demand,
        })
    }
}

#[derive(Debug)]
pub struct EbsPricingData {
    pub region_code: String,
    pub instance_type: String, // using same field for compatibility
    pub on_demand: HashMap<String, serde_json::Value>,
}

impl EbsPricingData {
    /// Extract EbsPricingData from AWS Pricing API JSON
    pub fn from_json(value: &Value) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let product = value
            .get("product")
            .and_then(|p| p.get("attributes"))
            .ok_or("Missing product.attributes")?;

        let region_code = extract_string_field(product, "regionCode")?;
        let instance_type = extract_string_field(product, "volumeApiName")?;

        let on_demand = value
            .get("terms")
            .and_then(|t| t.get("OnDemand"))
            .and_then(|od| od.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        Ok(EbsPricingData {
            region_code,
            instance_type,
            on_demand,
        })
    }
}

/// Helper function to extract required string fields from JSON
fn extract_string_field(
    obj: &Value,
    field: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    obj.get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing or invalid field: {}", field).into())
}

/// Helper function to extract optional string fields from JSON
fn extract_optional_string_field(obj: &Value, field: &str) -> Option<String> {
    obj.get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
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
            tenancy: details.tenancy,
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
