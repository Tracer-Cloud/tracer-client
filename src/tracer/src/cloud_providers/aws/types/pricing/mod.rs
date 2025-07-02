use serde_json::Value;

pub mod api;
pub mod aws;

pub(crate) use aws::ServiceCode;
pub use aws::{
    EBSFilterBuilder, EC2FilterBuilder, EbsPricingData, FilterableInstanceDetails, PricingData,
    VolumeMetadata,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstancePricingContext {
    pub ec2_pricing: FlattenedData,
    pub ebs_pricing: Option<FlattenedData>,
    pub total_hourly_cost: f64,
    pub source: String, // "Live" or "Static"
    pub cost_per_minute: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FlattenedData {
    pub instance_type: String,
    pub region_code: String,
    pub vcpu: String,
    pub memory: String,
    pub price_per_unit: f64,
    pub unit: String,

    // EBS-specific extensions
    pub price_per_gib: Option<f64>,
    pub price_per_iops: Option<f64>,
    pub price_per_throughput: Option<f64>,
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
        (0.0, "".to_string())
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
            // explicitly None for EBS-only fields
            price_per_gib: None,
            price_per_iops: None,
            price_per_throughput: None,
        }
    }

    pub fn flatten_ebs_data(data: &EbsPricingData) -> FlattenedData {
        let mut price_per_gib = None;
        let mut price_per_iops = None;
        let mut price_per_throughput = None;

        for value in data.on_demand.values() {
            if let Value::Object(term) = value {
                if let Some(Value::Object(price_dimensions)) = term.get("priceDimensions") {
                    for dim in price_dimensions.values() {
                        if let Value::Object(dim_map) = dim {
                            let desc = dim_map
                                .get("description")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_lowercase();

                            let unit = dim_map.get("unit").and_then(Value::as_str).unwrap_or("");
                            let price_str = dim_map
                                .get("pricePerUnit")
                                .and_then(|p| p.get("USD"))
                                .and_then(Value::as_str)
                                .unwrap_or("0");

                            let price = price_str.parse::<f64>().unwrap_or(0.0);

                            match (desc.as_str(), unit) {
                                (d, "GB-Mo") if d.contains("storage") => {
                                    price_per_gib = Some(price)
                                }
                                (d, "IOPS-Mo") if d.contains("iops") => {
                                    price_per_iops = Some(price)
                                }
                                (d, "MBps-Mo") if d.contains("throughput") => {
                                    price_per_throughput = Some(price)
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        FlattenedData {
            instance_type: data.instance_type.clone(),
            region_code: data.region_code.clone(),
            vcpu: String::new(),
            memory: String::new(),
            price_per_unit: price_per_gib.unwrap_or(0.0),
            unit: "GB-Mo".to_string(),
            price_per_gib,
            price_per_iops,
            price_per_throughput,
        }
    }
    /// Returns the EC2 price in USD per minute.
    /// Assumes price_per_unit is in USD per hour.
    pub fn price_per_minute(&self) -> f64 {
        self.price_per_unit / 60.0
    }
}
