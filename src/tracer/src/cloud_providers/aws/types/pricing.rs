use aws_sdk_pricing::types::{Filter as PricingFilters, FilterType as PricingFilterType};
use serde_json::Value;
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

#[derive(Debug, serde::Deserialize)]
pub struct OnDemandTerm {
    #[serde(rename = "priceDimensions", flatten)]
    pub price_dimensions: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FlattenedData {
    pub instance_type: String,
    pub region_code: String,
    pub vcpu: String,
    pub memory: String,
    pub price_per_unit: f64,
    pub unit: String,
}

impl FlattenedData {
    fn extract_price_info(value: &Value) -> (f64, String) {
        if let Value::Object(map) = value {
            if map.contains_key("unit") && map.contains_key("pricePerUnit") {
                let unit = map
                    .get("unit")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let price_per_unit = map
                    .get("pricePerUnit")
                    .and_then(|p| p.get("USD"))
                    .and_then(Value::as_str)
                    .unwrap_or("0.0");
                let price_per_unit = price_per_unit.parse::<f64>().unwrap_or(0.0);
                return (price_per_unit, unit);
            }

            for v in map.values() {
                let (price, unit) = Self::extract_price_info(v);
                if !unit.is_empty() {
                    return (price, unit);
                }
            }
        }
        (0.0, "".to_string()) // Default if not found
    }

    pub fn flatten_data(data: &PricingData) -> FlattenedData {
        let (price_per_unit, unit) = data
            .on_demand
            .values()
            .next()
            .map_or((0.0, "".to_string()), Self::extract_price_info);

        FlattenedData {
            instance_type: data.instance_type.clone(),
            region_code: data.region_code.clone(),
            vcpu: data.vcpu.clone(),
            memory: data.memory.clone(),
            price_per_unit,
            unit,
        }
    }

    /// Returns the EC2 price in USD per minute.
    /// Assumes price_per_unit is in USD per hour.
    pub fn price_per_minute(&self) -> f64 {
        self.price_per_unit / 60.0
    }
}

#[derive(Debug)]
pub struct EC2FilterBuilder {
    pub instance_type: String,
    pub region: String,
}

impl EC2FilterBuilder {
    /// "intance_type: InstanceType" E:g: t3.small
    // "region": "regionCode" "us-east-1"
    // Instance type and region code are enough to get the most precise pricing data
    pub fn to_filter(&self) -> Vec<PricingFilters> {
        vec![
            PricingFilters::builder()
                .field("InstanceType".to_string())
                .value(self.instance_type.to_owned())
                .r#type(PricingFilterType::TermMatch)
                .build()
                .expect("failed to build filters"),
            PricingFilters::builder()
                .field("regionCode".to_string())
                .value(self.region.to_owned())
                .r#type(PricingFilterType::TermMatch)
                .build()
                .expect("failed to build filter"),
        ]
    }
}
